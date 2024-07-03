use crate::module_struct::{ModuleHead, ModuleTail};
use super::{Converter, ModuleDataField, ModuleInputFormat, ModuleMsgConverter, ModuleOutputFormat};
use obd_coffee_maker_interface::msg::{CoffeeFeederInput, CoffeeFeederOutput};
use std::sync::{Arc, Mutex};
use anyhow::{Result, Error, anyhow};
use rumqttc::{Client, QoS};
use rclrs::{Node, Publisher, Subscription};

pub struct CoffeeFeederConverter{
    pub name: String,
    base_converter: ModuleMsgConverter,
    node: Arc<Node>,
    mqtt_client: Arc<Mutex<Client>>,
    pub ros_subscriber: Arc<Mutex<Option<Arc<Subscription<CoffeeFeederInput>>>>>,
    pub ros_publisher: Arc<Mutex<Option<Arc<Publisher<CoffeeFeederOutput>>>>>,
}

impl CoffeeFeederConverter {
    pub fn new(mqtt_client: Arc<Mutex<Client>>, node: Arc<Node>) -> Self { 
        let module_name = String::from("coffee_feeder");

        let input_format = ModuleInputFormat::new(
            ModuleHead::new(
                ModuleDataField::new(4, 0, "@COF".to_string()), 
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
                ModuleDataField::new(4, 0, "@COF".to_string()), 
                ModuleDataField::new(2, 4, "00".to_string()), 
                ModuleDataField::new(2, 6, "00".to_string()), 
                ModuleDataField::new(2, 8, "06".to_string())
            ), 
            ModuleDataField::new(1, 10, "0".to_string()),
            vec![ModuleDataField::new(5, 11, "00000".to_string())],
            ModuleTail::new(
                ModuleDataField::new(2, 16, "00".to_string()), 
                ModuleDataField::new(1, 18, "#".to_string())
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

impl Clone for CoffeeFeederConverter {
    fn clone(&self) -> Self {
        CoffeeFeederConverter {
            base_converter: self.base_converter.clone(),
            mqtt_client: self.mqtt_client.clone(),
            node: self.node.clone(),
            name: self.name.clone(),
            ros_subscriber: Arc::new(Mutex::new(self.ros_subscriber.lock().unwrap().clone())),
            ros_publisher: Arc::new(Mutex::new(self.ros_publisher.lock().unwrap().clone()))
        }
    }
}

impl Converter for CoffeeFeederConverter{
    type ModuleInput = CoffeeFeederInput;
    type ModuleOutput = CoffeeFeederOutput;

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

        let mut coffee_feeder_output = Self::ModuleOutput::default();

        let msg_validated: bool = self.base_converter.validate_get_str(mqtt_msg);
        if msg_validated {
            let full_payload_byte_str = self.base_converter.payload_from_full_output_format_string(mqtt_msg);
            let state_size = self.base_converter.output_format.state.size;
            match full_payload_byte_str[0..state_size].parse::<u8>() {
                Ok(num) => {
                    coffee_feeder_output.state = num;
                },
                Err(_) => {
                    eprintln!("[{}] cannot parse value of {} to u8", self.name, &full_payload_byte_str);
                    return None;
                }
            }
            let payload_byte_str = full_payload_byte_str[state_size..].to_string();
            let status_bin = self.base_converter.payload_to_binary_string(&payload_byte_str);
            coffee_feeder_output.capsule = self.base_converter.binary_string_to_int(&status_bin[0..2]);
            coffee_feeder_output.water_level = self.base_converter.binary_string_to_int(&status_bin[2..4]);
            coffee_feeder_output.water_filling = self.base_converter.binary_string_to_int(&status_bin[4..6]);
            coffee_feeder_output.coffee_feeder = self.base_converter.binary_string_to_int(&status_bin[8..12]);

            println!("[{}] decoded: {:#?}", self.name, coffee_feeder_output);
            println!("{}", &format!("[{}] ...... //DECODING MSG// .......\n\n", self.name));

            Some(coffee_feeder_output)

        } else {
            eprintln!("{}", &format!("[{}] unable to decoded msg", self.name));
            println!("{}", &format!("[{}] ...... //DECODING MSG// .......\n\n", self.name));
            
            None
        }
    }

    fn ros_2_mqtt(&self, ros_msg: &Self::ModuleInput) -> String {
        println!("{}", &format!("[{}] ...... ENCODING MSG .......", self.name));
        println!("{}", &format!("[{}] receive /input with command: {}, value: {}", self.name, ros_msg.command, ros_msg.value));

        let header_to_payload_str = self.base_converter.create_module_set_message(&ros_msg.command, &ros_msg.value);
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
    

    
