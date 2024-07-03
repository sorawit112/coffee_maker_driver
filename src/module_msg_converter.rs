use anyhow::Error;

pub use crate::module_struct::{ModuleDataField, ModuleOutputFormat, ModuleInputFormat};

pub mod coffee_feeder_converter;
pub mod capsule_feeder_converter;
pub mod cup_holder_converter;
pub mod tank_converter;
pub mod pdu_converter;
pub mod light_converter;

pub trait Converter {
    type ModuleInput;
    type ModuleOutput;

    fn start(&self) -> Result<(), Error>;

    fn ros_2_mqtt(&self, ros_msg: &Self::ModuleInput) -> String;

    fn mqtt_2_ros(&self, mqtt_msg: &str) -> Option<Self::ModuleOutput>;

    fn handle_mqtt_message(&self, topic: &str, payload: &str);
}


pub struct ModuleMsgConverter {
    // node: Node,
    module_name: String,
    // set_pub: Option<Publisher<String>>,
    // output_pub: Option<Publisher<String>>,
    // input_sub: Option<Subscription>,
    // get_sub: Option<Subscription>,
    // ros_input_type: ModuleInputMsgType,
    // ros_output_type: ModuleOutputMsgType,
    // ros2mqtt_cb_group: CallbackGroup,
    // mqtt2ros_cb_group: CallbackGroup,
    input_format: ModuleInputFormat,
    output_format: ModuleOutputFormat
}

impl Clone for ModuleMsgConverter{
    fn clone(&self) -> Self {
        ModuleMsgConverter {
            module_name: self.module_name.clone(),
            input_format: self.input_format.clone(),
            output_format: self.output_format.clone(),
        }
    }
}

impl ModuleMsgConverter {
    ////////////////////////////////////////////////////////////////////////////////
    ////               construction                                             ////
    ////////////////////////////////////////////////////////////////////////////////
    pub fn new(
        module_name : String,
        input_format: ModuleInputFormat,
        output_format: ModuleOutputFormat
    ) -> Self { 
        Self {module_name, input_format, output_format}
    }

    ////////////////////////////////////////////////////////////////////////////////
    ////               property                                                 ////
    ////////////////////////////////////////////////////////////////////////////////
    pub fn input_pkg_length(&self) -> usize {
        self.input_format.tail.end.index + 1
    }

    pub fn output_pkg_length(&self) -> usize {
        self.output_format.tail.end.index + 1
    }

    ////////////////////////////////////////////////////////////////////////////////
    ////               class functions                                          ////
    ////////////////////////////////////////////////////////////////////////////////
    pub fn payload_to_binary_string(&self, payload: &str) -> String {
        assert_eq!(payload.len(), 5, "payload length must be 5 char but input is '{:?}'", payload);
        format!("{:016b}", payload.parse::<u16>().unwrap()).chars().rev().collect::<String>()
    }

    pub fn binary_string_to_int(&self, bin_str: &str) -> u8 {
        let rev_str = bin_str.chars().rev().collect::<String>();
        u8::from_str_radix(rev_str.as_str(), 2).unwrap()
    }

    pub fn create_module_set_message(&self, cmd_int: &u8, val_int: &u8) -> String {
        format!(
            "{}{}{}{}{:0cmd_size$}{:0val_size$}",
            self.input_format.header().string,
            self.input_format.package().string,
            self.input_format.setting().string,
            self.input_format.length().string,
            cmd_int, val_int,
            cmd_size = self.input_format.command.size,
            val_size = self.input_format.value.size
        )
    }

    pub fn payload_from_full_output_format_string(&self, output_format_string: &str) -> String {
        let idx_start = self.output_format.state.index;
        let idx_end = self.output_format.lrc().index;
        output_format_string[idx_start..idx_end].to_string()
    } //output_mqtt_msg still valid after call this function hence borrow it

    fn calculate_lrc_from_string(&self, data: &str) -> String {
        // Step 1: Sum the ASCII values of the characters
        let sum_val: u32 = data.bytes().map(|b| b as u32).sum();
        
        // Step 2: Calculate the LRC value
        let lrc_i = (sum_val ^ 0xFF) + 1;

        // Step 3: Extract the higher and lower nibbles
        let lrc_0 = ((lrc_i >> 4) & 0x0F) as u8;
        let lrc_1 = (lrc_i & 0x0F) as u8;

        // Step 4: Convert nibbles to ASCII representation
        let lrc_0 = if lrc_0 < 10 {
            (lrc_0 + b'0') as char
        } else {
            (lrc_0 - 10 + b'A') as char
        };

        let lrc_1 = if lrc_1 < 10 {
            (lrc_1 + b'0') as char
        } else {
            (lrc_1 - 10 + b'A') as char
        };

        // Step 5: Return the concatenated result
        format!("{}{}", lrc_0, lrc_1)
    }

    fn validate_get_str(&self, msg: &str) -> bool {
        if self.output_pkg_length() != msg.len() {
            eprintln!("{}", &format!( 
                "[{}] {} format incorrect with output format [{}/{}]",
                self.module_name, msg, self.output_pkg_length(), msg.len()
            ));
            return false;
        }

        // Step 2: Extract data and calculate LRC
        let data: &str = &msg[..msg.len() - 1];
        let lrc = self.calculate_lrc_from_string(&data[..data.len() - 2]);

        // Extract the second last and last characters
        let second_last_char = data.chars().nth(data.len() - 2).unwrap();
        let last_char = data.chars().nth(data.len() - 1).unwrap();

        // Convert the second last character to its integer value
        let mut buf = if second_last_char < 'A' {
            (second_last_char as u8 - b'0') << 4
        } else {
            (second_last_char as u8 - b'A' + 10) << 4
        };

        // Convert the last character to its integer value and combine it with the buffer
        buf |= if last_char < 'A' {
            last_char as u8 - b'0'
        } else {
            last_char as u8 - b'A' + 10
        };

        // Step 4: Create new data string by replacing the last two characters with buffer
        let new_data = String::from(&data[..data.len() - 2]);
        // new_data.push(buf as char);

        // Step 5: Calculate checksum and validate it
        let mut sum_val: u32 = new_data.bytes().map(|b| b as u32).sum();
        sum_val += buf as u32;

        let sum_val_8bit: u32 = sum_val & 0xFF;

        if sum_val_8bit != 0 {
            eprintln!("{}", &format!(
                "[{}] Checksum not validated with input LRC: {} validate is LRC: {}",
                self.module_name, &msg[msg.len() - 3..msg.len() - 1],
                lrc
            ));
        }

        sum_val_8bit == 0
    }
}