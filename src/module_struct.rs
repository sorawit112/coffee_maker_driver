///////////////////////////////////////////////////////////
///                  Struct Defination                 ////
///////////////////////////////////////////////////////////

pub struct ModuleDataField {
    pub size: usize,
    pub index: usize,
    pub string: String
}

impl Clone for ModuleDataField{
    fn clone(&self) -> Self {
        ModuleDataField {
            size : self.size,
            index : self.index,
            string : self.string.clone(),
        }
    }
}

pub struct ModuleHead {
    pub header: ModuleDataField,
    pub package: ModuleDataField,
    pub setting: ModuleDataField,
    pub length: ModuleDataField,
}

impl Clone for ModuleHead{
    fn clone(&self) -> Self {
        ModuleHead {
            header : self.header.clone(),
            package : self.package.clone(),
            setting : self.setting.clone(),
            length : self.length.clone(),
        }
    }
}

pub struct ModuleTail {
    pub lrc: ModuleDataField,
    pub end: ModuleDataField,
}

impl Clone for ModuleTail{
    fn clone(&self) -> Self {
        ModuleTail {
            lrc : self.lrc.clone(),
            end : self.end.clone(),
        }
    }
}


pub struct ModuleInputFormat{
    pub head: ModuleHead,
    pub command: ModuleDataField,
    pub value: ModuleDataField,
    pub tail: ModuleTail
}

impl Clone for ModuleInputFormat{
    fn clone(&self) -> Self {
        ModuleInputFormat {
            head : self.head.clone(),
            command : self.command.clone(),
            value : self.value.clone(),
            tail : self.tail.clone()
        }
    }
}

pub struct ModuleOutputFormat{
    pub head: ModuleHead,
    pub state: ModuleDataField,
    pub payload: Vec<ModuleDataField>,
    pub tail: ModuleTail
}

impl Clone for ModuleOutputFormat{
    fn clone(&self) -> Self {
        ModuleOutputFormat {
            head : self.head.clone(),
            state : self.state.clone(),
            payload : self.payload.clone(),
            tail : self.tail.clone()
        }
    }
}

///////////////////////////////////////////////////////////
///                  Struct Implementation             ////
///////////////////////////////////////////////////////////
impl ModuleDataField {
    pub fn new(
        size: usize,
        index: usize,
        string: String,
    ) -> Self {
        Self {size, index, string}
    }
}

impl ModuleHead {
    pub fn new(
        header: ModuleDataField,
        package: ModuleDataField,
        setting: ModuleDataField,
        length: ModuleDataField,
    ) -> Self {
        Self {header, package, setting, length}
    }
}

impl ModuleTail {
    pub fn new(
        lrc: ModuleDataField,
        end: ModuleDataField,
    ) -> Self {
        Self {lrc, end}
    }
}


impl ModuleInputFormat{
    pub fn new(
        head: ModuleHead,
        command: ModuleDataField,
        value: ModuleDataField,
        tail: ModuleTail
    ) -> Self {
        Self {head, command, value, tail}
    }

    pub fn header(&self) -> &ModuleDataField {
        &self.head.header
    }

    pub fn package(&self) -> &ModuleDataField {
        &self.head.package
    }

    pub fn setting(&self) -> &ModuleDataField {
        &self.head.setting
    }

    pub fn length(&self) -> &ModuleDataField {
        &self.head.length
    }

    pub fn lrc(&self) -> &ModuleDataField {
        &self.tail.lrc
    } 

    pub fn end(&self) -> &ModuleDataField {
        &self.tail.end
    } 

    pub fn set_command(&mut self, command: u8) {
        self.command.string = command.to_string();
    }

    pub fn set_value(&mut self, value: u8) {
        self.value.string = value.to_string();
    }

}

impl ModuleOutputFormat{
    pub fn new(
        head: ModuleHead,
        state: ModuleDataField,
        payload: Vec<ModuleDataField>,
        tail: ModuleTail
    ) -> Self {
        Self {head, state, payload, tail}
    }

    pub fn header(&self) -> &ModuleDataField {
        &self.head.header
    }

    pub fn package(&self) -> &ModuleDataField {
        &self.head.package
    }

    pub fn setting(&self) -> &ModuleDataField {
        &self.head.setting
    }

    pub fn length(&self) -> &ModuleDataField {
        &self.head.length
    }

    pub fn lrc(&self) -> &ModuleDataField {
        &self.tail.lrc
    } 

    pub fn end(&self) -> &ModuleDataField {
        &self.tail.end
    } 
}
