use crate::module_struct::{ModuleHead, ModuleTail};
use super::{Converter, ModuleDataField, ModuleInputFormat, ModuleMsgConverter, ModuleOutputFormat};
use obd_coffee_maker_interface::msg::{CupHolderInput, CupHolderOutput};
use std::sync::{Arc, Mutex};
use anyhow::{Result, Error, anyhow};
use rumqttc::{Client, QoS};
use rclrs::{Node, Publisher, Subscription};

pub struct CupHolderConverter{
    pub name: String,
    base_converter: ModuleMsgConverter,
    node: Arc<Node>,
    mqtt_client: Arc<Mutex<Client>>,
    pub ros_subscriber: Arc<Mutex<Option<Arc<Subscription<CupHolderInput>>>>>,
    pub ros_publisher: Arc<Mutex<Option<Arc<Publisher<CupHolderOutput>>>>>,
}

impl CupHolderConverter {
    pub fn new(mqtt_client: Arc<Mutex<Client>>, node: Arc<Node>) -> Self { 
        let module_name = String::from("cup_holder");

        let input_format = ModuleInputFormat::new(
            ModuleHead::new(
                ModuleDataField::new(4, 0, "@CUP".to_string()), 
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
                ModuleDataField::new(4, 0, "@CUP".to_string()), 
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

impl Clone for CupHolderConverter {
    fn clone(&self) -> Self {
        CupHolderConverter {
            base_converter: self.base_converter.clone(),
            mqtt_client: self.mqtt_client.clone(),
            node: self.node.clone(),
            name: self.name.clone(),
            ros_subscriber: Arc::new(Mutex::new(self.ros_subscriber.lock().unwrap().clone())),
            ros_publisher: Arc::new(Mutex::new(self.ros_publisher.lock().unwrap().clone()))
        }
    }
}

impl Converter for CupHolderConverter{
    type ModuleInput = CupHolderInput;
    type ModuleOutput = CupHolderOutput;

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

        let mut cup_holder_output = Self::ModuleOutput::default();

        let msg_validated: bool = self.base_converter.validate_get_str(mqtt_msg);
        if msg_validated {
            let full_payload_byte_str = self.base_converter.payload_from_full_output_format_string(mqtt_msg);
            let state_size = self.base_converter.output_format.state.size;
            match full_payload_byte_str[0..state_size].parse::<u8>() {
                Ok(num) => {
                    cup_holder_output.state = num;
                },
                Err(_) => {
                    eprintln!("[{}] cannot parse value of {} to u8", self.name, &full_payload_byte_str);
                    return None;
                }
            }
            let status_size = self.base_converter.output_format.payload[0].size;
            let position_size = self.base_converter.output_format.payload[1].size;

            let status_byte_str = full_payload_byte_str[state_size..state_size+status_size].to_string();
            let position_byte_str = full_payload_byte_str[state_size+status_size..state_size+status_size+position_size].to_string();
            // mutable hence we will replace first index('0' or '-') by '0'
            let mut weight_byte_str = full_payload_byte_str[state_size+status_size+position_size..].to_string();
            
            let weight_signed = weight_byte_str.chars().next().unwrap();

            weight_byte_str.replace_range(0..1, "0");
        
            let status_bin = self.base_converter.payload_to_binary_string(&status_byte_str);
            let position_bin = self.base_converter.payload_to_binary_string(&position_byte_str);
            let weight_bin = self.base_converter.payload_to_binary_string(&weight_byte_str);
            
            // status payload
            let coffee_detect= (status_bin.chars().nth(0).unwrap() as u8 - b'0') == 1;
            let cup_detect = (status_bin.chars().nth(1).unwrap() as u8 - b'0') == 1;
            let water_detect = (status_bin.chars().nth(2).unwrap() as u8 - b'0') == 1;
            let ice_detect = (status_bin.chars().nth(3).unwrap() as u8 - b'0') == 1;
            let cup_pump = self.base_converter.binary_string_to_int(&status_bin[4..6]) == 1;
            let cup_stock = self.base_converter.binary_string_to_int(&status_bin[6..8]) as u8;

            // position payload
            let position = self.base_converter.binary_string_to_int(&position_bin[..]);

            // weight payload 
            let mut weight = self.base_converter.binary_string_to_int(&weight_bin[..]) as i16;

            if weight_signed == '-'{
                weight *= -1
            }
            
            cup_holder_output.coffee_detect = coffee_detect;
            cup_holder_output.cup_detect = cup_detect;
            cup_holder_output.water_detect = water_detect;
            cup_holder_output.ice_detect = ice_detect;
            cup_holder_output.cup_pump = cup_pump; 
            cup_holder_output.cup_stock = cup_stock; 
            cup_holder_output.position = position ;
            cup_holder_output.weight = weight;
        
            println!("[{}] decoded: {:#?}", self.name, cup_holder_output);
            println!("{}", &format!("[{}] ...... //DECODING MSG// .......\n\n", self.name));

            Some(cup_holder_output)

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
    

    
