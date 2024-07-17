use crate::module_struct::{ModuleHead, ModuleTail};
use super::{Converter, ModuleDataField, ModuleInputFormat, ModuleMsgConverter, ModuleOutputFormat};
use obd_coffee_maker_interface::msg::{PDUInput, PDUOutput};
use std::sync::{Arc, Mutex};
use anyhow::{Result, Error, anyhow};
use rumqttc::{Client, QoS};
use rclrs::{Node, Publisher, Subscription};

pub struct PDUConverter{
    pub name: String,
    base_converter: ModuleMsgConverter,
    node: Arc<Node>,
    mqtt_client: Arc<Mutex<Client>>,
    pub ros_subscriber: Arc<Mutex<Option<Arc<Subscription<PDUInput>>>>>,
    pub ros_publisher: Arc<Mutex<Option<Arc<Publisher<PDUOutput>>>>>,
}

impl PDUConverter {
    pub fn new(mqtt_client: Arc<Mutex<Client>>, node: Arc<Node>) -> Self { 
        let module_name = String::from("pdu");

        let input_format = ModuleInputFormat::new(
            ModuleHead::new(
                ModuleDataField::new(4, 0, "@PDU".to_string()), 
                ModuleDataField::new(2, 4, "00".to_string()), 
                ModuleDataField::new(2, 6, "00".to_string()), 
                ModuleDataField::new(2, 8, "07".to_string())
            ), 
            ModuleDataField::new(2, 10, "00".to_string()), 
            ModuleDataField::new(5, 12, "00000".to_string()), 
            ModuleTail::new(
                ModuleDataField::new(2, 17, "00".to_string()), 
                ModuleDataField::new(1, 19, "#".to_string())
            )
        );

        let output_format = ModuleOutputFormat::new(
            ModuleHead::new(
                ModuleDataField::new(4, 0, "@PDU".to_string()), 
                ModuleDataField::new(2, 4, "00".to_string()), 
                ModuleDataField::new(2, 6, "00".to_string()), 
                ModuleDataField::new(2, 8, "16".to_string())
            ), 
            ModuleDataField::new(1, 10, "0".to_string()),
            vec![ModuleDataField::new(5, 11, "00000".to_string()), 
                         ModuleDataField::new(5, 16, "00000".to_string()),
                         ModuleDataField::new(5, 21, "00000".to_string())],
            ModuleTail::new(
                ModuleDataField::new(2, 26, "00".to_string()), 
                ModuleDataField::new(1, 28, "#".to_string())
            )
        );

        let base_converter = ModuleMsgConverter::new(module_name.clone(), input_format, output_format);
        
        Self {
            name: module_name,
            base_converter,
            node,
            mqtt_client,
            ros_subscriber: Arc::new(Mutex::new(None)),
            ros_publisher: Arc::new(Mutex::new(None)),
        }
    }

}

impl Clone for PDUConverter {
    fn clone(&self) -> Self {
        PDUConverter {
            base_converter: self.base_converter.clone(),
            mqtt_client: self.mqtt_client.clone(),
            node: self.node.clone(),
            name: self.name.clone(),
            ros_subscriber: Arc::new(Mutex::new(self.ros_subscriber.lock().unwrap().clone())),
            ros_publisher: Arc::new(Mutex::new(self.ros_publisher.lock().unwrap().clone()))
        }
    }
}

impl Converter for PDUConverter{
    type ModuleInput = PDUInput;
    type ModuleOutput = PDUOutput;

    fn start(&self) -> Result<(), Error> {
        let mqtt_client = self.mqtt_client.clone();
        let node = self.node.clone();
        let namespace = self.name.clone();
        let self_clone = self.clone();

        // ROS to MQTT conversion
        let ros_sub = node.create_subscription::<Self::ModuleInput, _>(
            &format!("{}/input", namespace),
            rclrs::QOS_PROFILE_DEFAULT,
            move |msg| {
                let payload = self_clone.ros_2_mqtt(&msg);
                if let Ok(mut client) = mqtt_client.lock() {
                    if let Err(e) = client.publish(&format!("{}/set", namespace), QoS::AtLeastOnce, false, payload) {
                        eprintln!("[{}] Failed to publish MQTT message: {:?}", namespace, e);
                    }
                } else {
                    eprintln!("[{}] Failed to acquire MQTT client lock", namespace);
                }
            },
        )?;

        if let Ok(mut subscriber_guard) = self.ros_subscriber.lock() {
            *subscriber_guard = Some(ros_sub);
        } else {
            eprintln!("[{}] Failed to acquire lock for ros_publisher", self.name);
        }

        // MQTT to ROS conversion
        let ros_pub = node.create_publisher::<Self::ModuleOutput>(
            &format!("{}/output", self.name),
            rclrs::QOS_PROFILE_DEFAULT
        )?;

        // Store the publisher
        
        if let Ok(mut publisher_guard) = self.ros_publisher.lock() {
            *publisher_guard = Some(ros_pub);
        } else {
            eprintln!("Failed to acquire lock for ros_publisher");
        }

        let mqtt_client = self.mqtt_client.clone();
        if let Ok(mut client) = mqtt_client.lock() {
            if let Err(e) = client.subscribe(&format!("{}/get", self.name), QoS::AtLeastOnce) {
                eprintln!("[{}] Failed to subscribe to MQTT topic: {:?}", self.name, e);
                return Err(anyhow!(e));
            }
        } else {
            eprintln!("[{}] Failed to acquire MQTT client lock for subscription", self.name);
            return Err(anyhow!("Failed to acquire MQTT client lock"));
        }

        Ok(())
    }

    fn mqtt_2_ros(&self, mqtt_msg: &str) -> Option<Self::ModuleOutput> {
        println!("{}", &format!("[{}] ...... DECODING MSG .......", self.name));
        println!("{}", &format!("[{}] receive /get string: {}", self.name, mqtt_msg));

        let mut pdu_output = Self::ModuleOutput::default();

        let msg_validated: bool = self.base_converter.validate_get_str(mqtt_msg);
        if msg_validated {
            let full_payload_byte_str = self.base_converter.payload_from_full_output_format_string(mqtt_msg);
            let state_size = self.base_converter.output_format.state.size;
            match full_payload_byte_str[0..state_size].parse::<u8>() {
                Ok(num) => {
                    pdu_output.state = num;
                },
                Err(_) => {
                    eprintln!("[{}] cannot parse value of {} to u8", self.name, &full_payload_byte_str);
                    return None;
                }
            }
            let status_size = self.base_converter.output_format.payload[0].size;
            let voltage_size = self.base_converter.output_format.payload[1].size;

            let status_byte_str = full_payload_byte_str[state_size..state_size+status_size].to_string();
            let voltage_byte_str = full_payload_byte_str[state_size+status_size..state_size+status_size+voltage_size].to_string();
            let current_byte_str = full_payload_byte_str[state_size+status_size+voltage_size..].to_string();
        
            let status_bin = self.base_converter.payload_to_binary_string(&status_byte_str);
            let voltage_bin = self.base_converter.payload_to_binary_string(&voltage_byte_str);
            let current_bin = self.base_converter.payload_to_binary_string(&current_byte_str);
            
            // status payload
            let coffee_pwr = (status_bin.chars().nth(0).unwrap() as u8 - b'0') == 1;
            let caps_pwr = (status_bin.chars().nth(1).unwrap() as u8 - b'0') == 1;
            let cups_pwr = (status_bin.chars().nth(2).unwrap() as u8 - b'0') == 1;
            let tank_pwr = (status_bin.chars().nth(3).unwrap() as u8 - b'0') == 1;
            let light_pwr = (status_bin.chars().nth(4).unwrap() as u8 - b'0') == 1;
            
            // voltage payload
            let voltage = self.base_converter.binary_string_to_int(&voltage_bin[..]);

            // current payload 
            let current = self.base_converter.binary_string_to_int(&current_bin[..]);
            
            pdu_output.coffee_pwr = coffee_pwr;
            pdu_output.capsule_pwr = caps_pwr;
            pdu_output.cup_pwr = cups_pwr;
            pdu_output.tank_pwr = tank_pwr;
            pdu_output.light_pwr = light_pwr;
            pdu_output.voltage = voltage;
            pdu_output.current = current;
        
            println!("[{}] decoded: {:#?}", self.name, pdu_output);
            println!("{}", &format!("[{}] ...... //DECODING MSG// .......\n\n", self.name));

            Some(pdu_output)

        } else {
            eprintln!("{}", &format!("[{}] unable to decoded msg", self.name));
            println!("{}", &format!("[{}] ...... //DECODING MSG// .......\n\n", self.name));
            
            None
        }
    }

    fn ros_2_mqtt(&self, ros_msg: &Self::ModuleInput) -> String {
        println!("{}", &format!("[{}] ...... ENCODING MSG .......", self.name));
        println!("{}", &format!("[{}] receive /input with command: {}, value: {}", self.name, ros_msg.command, ros_msg.value));

        let header_to_payload_str = self.base_converter.create_module_set_message(ros_msg.command, ros_msg.value);
        let lrc = self.base_converter.calculate_lrc_from_string(&header_to_payload_str);
        println!("{}", &format!("[{}] content: {header_to_payload_str} LRC: {lrc}", self.name));

        let mqtt_string = format!("{}{}{}", header_to_payload_str, lrc, self.base_converter.input_format.end().string);

        println!("[{}] encoded: {}", self.name, mqtt_string);
        println!("{}", &format!("[{}] ...... //ENCODING MSG// .......\n\n", self.name));

        mqtt_string
    }

    fn handle_mqtt_message(&self, topic: &str, payload: &str) {
        if topic == format!("{}/get", self.name) {
            if let Some(ros_msg) = self.mqtt_2_ros(payload){
                if let Some(publisher) = self.ros_publisher.lock().unwrap().as_ref(){
                    publisher.publish(ros_msg).unwrap();
                }
            }       
            
        }
    }
}
    

    
