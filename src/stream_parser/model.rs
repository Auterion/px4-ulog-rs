use super::model_helper::{FlattenedFieldTypeMatcher, LittleEndianParser};
use std::collections::HashMap;
use std::marker::PhantomData;

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

pub struct ULogMessage<'a> {
    msg_type: u8,
    pub data: &'a [u8],
}

impl<'a> ULogMessage<'a> {
    // Returns the # bytes consumed
    //pub fn parse(data: &'a [u8]) -> (Option<Self>, usize) {}

    pub fn new(msg_type: u8, data: &'a [u8]) -> Self {
        if data.len() > u16::max_value() as usize {
            panic!("slice is too long");
        }
        Self { msg_type, data }
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
        self.data.len() as u16
    }

    pub fn data(&self) -> &'a [u8] {
        self.data
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FlattenedFieldType {
    Int8,
    UInt8,
    Int16,
    UInt16,
    Int32,
    UInt32,
    Int64,
    UInt64,
    Float,
    Double,
    Bool,
    Char,
}

#[derive(Clone, Debug)]
pub enum FlattenedFieldValue {
    Int8(i8),
    UInt8(u8),
    Int16(i16),
    UInt16(u16),
    Int32(i32),
    UInt32(u32),
    Int64(i64),
    UInt64(u64),
    Float(f32),
    Double(f64),
    Bool(bool),
    Char(char),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct MultiId(u8);

impl MultiId {
    pub fn new(value: u8) -> Self {
        Self(value)
    }
    pub fn value(&self) -> u8 {
        self.0
    }
}

#[derive(Clone, Debug)]
pub struct FlattenedField {
    pub flattened_field_name: String,
    pub field_type: FlattenedFieldType,
    pub offset: u16, // relative to the beginning of the message ()
}

#[derive(Clone, Debug, PartialEq)]
pub enum TimestampFieldType {
    UInt8,
    UInt16,
    UInt32,
    UInt64,
}

#[derive(Clone, Debug)]
pub struct TimestampField {
    pub field_type: TimestampFieldType,
    pub offset: u16, // relative to the beginning of the message ()
}

impl TimestampField {
    pub fn parse_timestamp(&self, data: &[u8]) -> u64 {
        match self.field_type {
            TimestampFieldType::UInt8 => u8::parse(&data[self.offset as usize..]) as u64,
            TimestampFieldType::UInt16 => u16::parse(&data[self.offset as usize..]) as u64,
            TimestampFieldType::UInt32 => u32::parse(&data[self.offset as usize..]) as u64,
            TimestampFieldType::UInt64 => u64::parse(&data[self.offset as usize..]),
        }
    }
}

#[derive(Debug)]
pub enum FieldLookupError {
    MissingField,
    TypeMismatch,
}

#[derive(Debug)]
pub struct UlogParseError {
    error_type: ParseErrorType,
    description: String,
}

impl UlogParseError {
    pub fn new(error_type: ParseErrorType, description: &str) -> Self {
        Self {
            error_type,
            description: description.to_string(),
        }
    }
}

#[derive(Debug)]
pub enum ParseErrorType {
    InvalidFile,
    Other,
}

#[derive(Clone, Debug)]
pub struct FlattenedFormat {
    pub message_name: String,
    pub fields: Vec<FlattenedField>,
    name_to_field: HashMap<String, FlattenedField>,
    pub timestamp_field: Option<TimestampField>,
    size: u16,
}

pub trait ParseableFieldType: LittleEndianParser + FlattenedFieldTypeMatcher {}

// Universal impl
impl<T: LittleEndianParser + FlattenedFieldTypeMatcher> ParseableFieldType for T {}

impl FlattenedFormat {
    pub fn new(
        message_name: String,
        fields: Vec<FlattenedField>,
        size: u16,
    ) -> Result<Self, UlogParseError> {
        let name_to_field: HashMap<String, FlattenedField> = fields
            .iter()
            .map(|f| (f.flattened_field_name.to_string(), (*f).clone()))
            .collect();
        let timestamp_field = name_to_field
            .get("timestamp")
            .and_then(|field| match field.field_type {
                FlattenedFieldType::UInt8 => Some(TimestampField {
                    field_type: TimestampFieldType::UInt8,
                    offset: field.offset,
                }),
                FlattenedFieldType::UInt16 => Some(TimestampField {
                    field_type: TimestampFieldType::UInt16,
                    offset: field.offset,
                }),
                FlattenedFieldType::UInt32 => Some(TimestampField {
                    field_type: TimestampFieldType::UInt32,
                    offset: field.offset,
                }),
                FlattenedFieldType::UInt64 => Some(TimestampField {
                    field_type: TimestampFieldType::UInt64,
                    offset: field.offset,
                }),
                _ => None,
            });
        Ok(Self {
            message_name,
            fields,
            name_to_field,
            timestamp_field,
            size,
        })
    }

    pub fn get_field_offset(
        &self,
        flattened_field_name: &str,
        field_type: FlattenedFieldType,
    ) -> Result<u16, FieldLookupError> {
        if let Some(field) = self.name_to_field.get(flattened_field_name) {
            if field.field_type == field_type {
                Ok(field.offset)
            } else {
                Err(FieldLookupError::TypeMismatch)
            }
        } else {
            Err(FieldLookupError::MissingField)
        }
    }

    pub fn get_field_parser<T: ParseableFieldType>(
        &self,
        flattened_field_name: &str,
    ) -> Result<FieldParser<T>, FieldLookupError> {
        if let Some(field) = self.name_to_field.get(flattened_field_name) {
            if T::matches(&field.field_type) {
                Ok(FieldParser::<T> {
                    offset: field.offset,
                    _phantom: PhantomData,
                })
            } else {
                Err(FieldLookupError::TypeMismatch)
            }
        } else {
            Err(FieldLookupError::MissingField)
        }
    }

    pub fn field_iter(&self) -> std::slice::Iter<FlattenedField> {
        self.fields.iter()
    }

    pub fn message_name(&self) -> &str {
        &self.message_name
    }

    pub fn size(&self) -> u16 {
        self.size
    }
}

pub struct FieldParser<T: ParseableFieldType> {
    offset: u16, // relative to the beginning of the message ()
    _phantom: PhantomData<T>,
}

impl<T: ParseableFieldType> FieldParser<T> {
    // data e.g. looks like the member in the DataMessage
    pub fn parse(&self, data: &[u8]) -> T {
        return T::parse(&data[(self.offset as usize)..]);
    }
    pub fn offset(&self) -> u16 {
        self.offset
    }
}

pub struct DataMessage<'a> {
    pub msg_id: u16,
    pub multi_id: MultiId,
    pub flattened_format: &'a FlattenedFormat,
    pub data: &'a [u8], // this includes the bytes of the msg_id.
}

#[derive(Debug)]
pub enum LogStage {
    Definitions,
    Data,
}

#[derive(Debug)]
pub enum ParameterMessage<'a> {
    Float(&'a str, f32, LogStage),
    Int32(&'a str, i32, LogStage),
}

pub struct LoggedStringMessage<'a> {
    pub log_level: u8,
    pub timestamp: u64,
    pub logged_message: &'a str,
}

impl<'a> LoggedStringMessage<'a> {
    pub fn human_readable_log_level(&self) -> &'static str {
        match self.log_level as char {
            '0' => "EMERGENCY",
            '1' => "ALERT",
            '2' => "CRITICAL",
            '3' => "ERROR",
            '4' => "WARNING",
            '5' => "NOTICE",
            '6' => "INFO",
            '7' => "DEBUG",
            _ => "UKNOWN",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_u32() {
        let mut data: [u8; 256] = [0; 256];
        data[13] = 1;
        let field = FlattenedField {
            flattened_field_name: "timestamp".to_string(),
            field_type: FlattenedFieldType::UInt32,
            offset: 10, // relative to the beginning of the message ()
        };
        let flattened_format =
            FlattenedFormat::new("message".to_string(), vec![field.clone()], 500).unwrap();
        let data_msg = DataMessage {
            msg_id: 1,
            multi_id: MultiId(10),
            flattened_format: &flattened_format,
            data: &data,
        };
        let parser = data_msg
            .flattened_format
            .get_field_parser::<u32>("timestamp")
            .expect("could not get parser");
        assert_eq!(10, parser.offset());
        assert_eq!(0x01000000, parser.parse(&data));
    }

}
