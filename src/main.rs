#![allow(dead_code)]
#![allow(unused)]

use coffee_maker_driver::module_msg_converter::{
    capsule_feeder_converter::CapsuleFeederConverter, coffee_feeder_converter::CoffeeFeederConverter, cup_holder_converter, light_converter, pdu_converter, tank_converter, Converter
};

use obd_coffee_maker_interface::msg::{
    CoffeeFeederInput, CoffeeFeederOutput,
    CapsuleFeederInput, CapsuleFeederOutput,
    CupHolderInput, CupHolderOutput,
    TankInput, TankOutput,
    PDUInput, PDUOutput,
    LightInput, LightOutput};

use std::{env, sync::{Arc, Mutex}};
use anyhow::{Error, Result};
use std::collections::HashMap;
use rclrs::{Node, Context, Subscription};
use std::thread;
use std::time::Duration;
use tokio::task;
use rumqttc::{MqttOptions, Client, QoS, Event, Packet};

enum ConvertersEnum {
    CoffeeFeeder(CoffeeFeederConverter),
    CapsuleFeeder(CapsuleFeederConverter)
}

impl Clone for ConvertersEnum {
    fn clone(&self) -> ConvertersEnum {
        match self {
            ConvertersEnum::CoffeeFeeder(coffee_feeder) => ConvertersEnum::CoffeeFeeder(coffee_feeder.clone()),
            ConvertersEnum::CapsuleFeeder(capsule_feeder) => ConvertersEnum::CapsuleFeeder(capsule_feeder.clone()),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>>{
    let ctx = Context::new(env::args())?;
    let node = rclrs::create_node(&ctx, "coffee_machine_driver")?;

    let mqtt_options = MqttOptions::new("converter_client", "localhost", 1883);
    let (mqtt_client, mut mqtt_connection) = Client::new(mqtt_options, 10);

    let mqtt_client = Arc::new(Mutex::new(mqtt_client));

    let coffee_feeder = CoffeeFeederConverter::new(mqtt_client.clone(), node.clone());
    let capsule_feeder  = CapsuleFeederConverter::new(mqtt_client.clone(), node.clone());

    let mut converters: HashMap<String, ConvertersEnum> = HashMap::new();

    coffee_feeder.start()?;
    capsule_feeder.start()?;

    converters.insert(coffee_feeder.name.clone(), ConvertersEnum::CoffeeFeeder(coffee_feeder));
    converters.insert(capsule_feeder.name.clone(), ConvertersEnum::CapsuleFeeder(capsule_feeder));
    

    let converters_clone = converters.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            loop {
                if let Ok(Event::Incoming(Packet::Publish(publish))) = mqtt_connection.eventloop.poll().await {
                    let topic = publish.topic.clone();

                    // Extract namespace from topic
                    let parts: Vec<&str> = topic.split('/').collect();
                    if parts.len() >= 2 && parts[1] == "get" {
                        let namespace = parts[0].to_string();
                        
                        // Publish to ROS using the stored publisher
                        if let Some(converter_enum) = converters_clone.get(&namespace) {
                            if let Ok(payload) = String::from_utf8(publish.payload.to_vec()){
                                match converter_enum {
                                    ConvertersEnum::CoffeeFeeder(c) => c.handle_mqtt_message(&topic, &payload),
                                    ConvertersEnum::CapsuleFeeder(c) => c.handle_mqtt_message(&topic, &payload),
                                }
                            } else {
                                eprint!("cannot convert bytes to String");
                            }
                        } else {
                            eprint!("incorrect format 'name/get' mqtt topic: {}", topic);
                        }
                    }
                }
            }
        });
    });

    rclrs::spin(node).unwrap();

    Ok(())
    
}