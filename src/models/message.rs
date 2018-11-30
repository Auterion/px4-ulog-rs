#[derive(Debug, PartialEq)]
pub enum MessageType {
    Unknown,
    Format,
    Data,
    Info,
    MultipleInfo,
    Parameter,
    AddLoggedMessage,
    RemoveLoggedMessage,
    Sync,
    Dropout,
    Logging,
    FlagBits,
}

pub struct ULogMessage {
    msg_type: u8,
    msg_size: u16,
    msg_pos: u64,
}

impl ULogMessage {
    pub fn new(msg_type: u8, msg_size: u16, msg_pos: u64) -> Self {
        Self {
            msg_type,
            msg_size,
            msg_pos,
        }
    }

    pub fn msg_type(&self) -> MessageType {
        match self.msg_type as char {
            'F' => MessageType::Format,
            'D' => MessageType::Data,
            'I' => MessageType::Info,
            'M' => MessageType::MultipleInfo,
            'P' => MessageType::Parameter,
            'A' => MessageType::AddLoggedMessage,
            'R' => MessageType::RemoveLoggedMessage,
            'S' => MessageType::Sync,
            'O' => MessageType::Dropout,
            'L' => MessageType::Logging,
            'B' => MessageType::FlagBits,
            _ => MessageType::Unknown,
        }
    }

    pub fn size(&self) -> u16 {
        self.msg_size
    }

    pub fn position(&self) -> u64 {
        self.msg_pos
    }
}
