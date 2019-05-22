use crate::stream_parser::model::DataMessage;
use crate::stream_parser::model::FlattenedField;
use crate::stream_parser::model::FlattenedFieldValue;
pub use crate::stream_parser::model::{FlattenedFieldType, MultiId};
use crate::stream_parser::LittleEndianParser;
use crate::stream_parser::LogParser;
use std::collections::HashMap;
use std::io::Read;

pub struct ParsedData {
    pub messages: HashMap<String, HashMap<MultiId, HashMap<String, SomeVec>>>,
}

pub fn read_file(file_path: &str) -> Result<ParsedData, std::io::Error> {
    let mut f = std::fs::File::open(file_path)?;

    let mut reader = TotalArrayReader::create();
    let mut callback = |msg: &DataMessage| {
        reader.add_message(msg);
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut callback);

    const READ_START: usize = 64 * 1024;
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let num_bytes_read = f.read(&mut buf[READ_START..])?;
        if num_bytes_read == 0 {
            break;
        }
        parser
            .consume_bytes(&buf[READ_START..(READ_START + num_bytes_read)])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("err: {:?}", e)))?;
    }
    let data_format = parser.get_final_data_format();

    let mut messages = HashMap::<String, HashMap<MultiId, HashMap<String, SomeVec>>>::new();
    for msg_id in 0..reader.messages.len() {
        let fields = &mut reader.messages[msg_id];
        let msg_id = msg_id as u16;
        if let Some(description) = data_format.get_message_description(msg_id) {
            if fields.is_empty() {
                continue;
            }
            if description.0.fields.len() != fields.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Mismatch between schema and reality",
                ));
            }
            let field_map = messages
                .entry(description.0.message_name.to_string())
                .or_default()
                .entry(description.1.clone())
                .or_default();
            for (field_index, field) in fields.drain(0..).enumerate() {
                field_map.insert(
                    description.0.fields[field_index]
                        .flattened_field_name
                        .to_string(),
                    field,
                );
            }
        }
    }

    Ok(ParsedData { messages })
}

#[derive(Clone, Debug)]
pub enum SomeVec {
    Int8(Vec<i8>),
    UInt8(Vec<u8>),
    Int16(Vec<i16>),
    UInt16(Vec<u16>),
    Int32(Vec<i32>),
    UInt32(Vec<u32>),
    Int64(Vec<i64>),
    UInt64(Vec<u64>),
    Float(Vec<f32>),
    Double(Vec<f64>),
    Bool(Vec<bool>),
    Char(Vec<char>),
}

macro_rules! vec_push_matcher {
    ($self_i:ident, $value:ident, $( $type:tt ),*) => (
        match $value {
            $(FlattenedFieldValue::$type(v) => {
                if let SomeVec::$type(vec) = $self_i{
                    vec.push(*v);
                }
                else {
                    panic!("SomeVec push types did not match: {:?}, {:?}" , $value, $self_i);
                }
            },)+
        }
    )
}

impl SomeVec {
    fn push(&mut self, value: &FlattenedFieldValue) {
        vec_push_matcher!(
            self, value, Int8, UInt8, Int16, UInt16, Int32, UInt32, Int64, UInt64, Float, Double,
            Bool, Char
        );
    }
}

macro_rules! vec_creation_matcher {
    ($value:ident, $( $type:tt ),*) => (
        match $value {
            $(FlattenedFieldType::$type => {
                SomeVec::$type(Vec::new())
            },)+
        }
    )
}

fn make_initial_vec(flattened_field_type: &FlattenedFieldType) -> SomeVec {
    vec_creation_matcher!(
        flattened_field_type,
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
        Char
    )
}

fn deserialize_field(field: &FlattenedField, mut serialized: &[u8]) -> FlattenedFieldValue {
    serialized = &serialized[field.offset as usize..];
    match field.field_type {
        FlattenedFieldType::Int8 => FlattenedFieldValue::Int8(i8::parse(serialized)),
        FlattenedFieldType::UInt8 => FlattenedFieldValue::UInt8(u8::parse(serialized)),
        FlattenedFieldType::Int16 => FlattenedFieldValue::Int16(i16::parse(serialized)),
        FlattenedFieldType::UInt16 => FlattenedFieldValue::UInt16(u16::parse(serialized)),
        FlattenedFieldType::Int32 => FlattenedFieldValue::Int32(i32::parse(serialized)),
        FlattenedFieldType::UInt32 => FlattenedFieldValue::UInt32(u32::parse(serialized)),
        FlattenedFieldType::Int64 => FlattenedFieldValue::Int64(i64::parse(serialized)),
        FlattenedFieldType::UInt64 => FlattenedFieldValue::UInt64(u64::parse(serialized)),
        FlattenedFieldType::Float => FlattenedFieldValue::Float(f32::parse(serialized)),
        FlattenedFieldType::Double => FlattenedFieldValue::Double(f64::parse(serialized)),
        FlattenedFieldType::Bool => FlattenedFieldValue::Bool(bool::parse(serialized)),
        FlattenedFieldType::Char => FlattenedFieldValue::Char(char::parse(serialized)),
    }
}

struct TotalArrayReader {
    messages: Vec<Vec<SomeVec>>,
}

impl TotalArrayReader {
    fn create() -> TotalArrayReader {
        let messages = Vec::new();
        Self { messages }
    }

    fn add_message(&mut self, msg: &DataMessage) {
        if msg.msg_id as usize >= self.messages.len() {
            self.messages.resize(msg.msg_id as usize + 1, Vec::new());
        }
        let field_values = &mut self.messages[msg.msg_id as usize];
        if field_values.is_empty() {
            field_values.reserve(msg.flattened_format.fields.len() + 1);
            for field in &msg.flattened_format.fields {
                field_values.push(make_initial_vec(&field.field_type))
            }
        }

        for i in 0..msg.flattened_format.fields.len() {
            let el = &msg.flattened_format.fields[i];
            let value = deserialize_field(&el, msg.data);
            field_values[i].push(&value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_log_file() {
        let filename = format!(
            "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
            env!("CARGO_MANIFEST_DIR")
        );
        read_file(&filename).unwrap();
    }

    #[test]
    fn reads_other_log_file() {
        let filename = format!("{}/tests/fixtures/sample.ulg", env!("CARGO_MANIFEST_DIR"));
        read_file(&filename).unwrap();
    }

    #[test]
    fn reads_repeated_submessage() {
        let filename = format!(
            "{}/tests/fixtures/esc_status_log.ulg",
            env!("CARGO_MANIFEST_DIR")
        );
        let parsed_data = read_file(&filename).unwrap();
        let msg = parsed_data
            .messages
            .get("esc_status")
            .unwrap()
            .get(&MultiId::new(0))
            .unwrap();

        assert!(msg.contains_key("esc[5].esc_rpm"));
    }
}
