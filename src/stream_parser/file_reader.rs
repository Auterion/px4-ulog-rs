use std::borrow::BorrowMut;
use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Read;
use std::iter::FromIterator;
use std::ops::DerefMut;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::model;
use crate::unpack;

use self::model::{DataMessage, FlattenedField, FlattenedFieldType, FlattenedFormat, MultiId};

#[derive(Debug, PartialEq)]
enum ParseStatus {
    Beginning,
    AfterHeader,
    InDefinitions,
    InData,
    //TODO: appends, probably InData works too
}

impl Default for ParseStatus {
    fn default() -> Self {
        ParseStatus::Beginning
    }
}

#[derive(Default)]
pub struct DataFormat {
    flattened_format: HashMap<String, FlattenedFormat>,
    registered_messages: HashMap<u16, (FlattenedFormat, MultiId)>,
}

impl DataFormat {
    fn new(flattened_format: HashMap<String, FlattenedFormat>) -> Self {
        DataFormat {
            flattened_format,
            ..Default::default()
        }
    }

    fn register_msg_id(
        &mut self,
        msg_id: u16,
        message_name: &str,
        multi_id: u8,
    ) -> Result<(), UlogParseError> {
        if let Some(flattened_message) = self.flattened_format.get(message_name) {
            if let Some(preexisting_message) = self
                .registered_messages
                .insert(msg_id, (flattened_message.clone(), MultiId::new(multi_id)))
            {
                return Err(UlogParseError::new(
                    ParseErrorType::Other,
                    &format!(
                        "duplicate registration for msg_id {:?}, initial one:\n{:#?}\nlater one:\n{:#?}",
                        msg_id,
                        preexisting_message,
                        flattened_message
                    ),
                ));
            }
            Ok(())
        } else {
            Err(UlogParseError::new(
                ParseErrorType::Other,
                &format!(
                    "Could not find format definition for message {}",
                    message_name
                ),
            ))
        }
    }

    // This should actually never return None
    pub fn get_message_description(&self, msg_id: u16) -> Option<&(FlattenedFormat, MultiId)> {
        self.registered_messages.get(&msg_id)
    }
}

#[derive(Default)]
pub struct LogParser<'c> {
    data_message_callback: Option<&'c mut FnMut(&model::DataMessage)>,
    logged_string_message_callback: Option<&'c mut FnMut(&model::LoggedStringMessage)>,
    version: u8,
    timestamp: u64,
    leftover: Vec<u8>,
    message_formats: HashMap<String, Vec<Field>>,
    flattened_format: DataFormat,
    status: ParseStatus,
}

const MAX_MESSAGE_SIZE: usize = 2 + 1 + (u16::max_value() as usize);
const HEADER_BYTES: [u8; 7] = [85, 76, 111, 103, 1, 18, 53];

#[derive(Debug)]
pub struct UlogParseError {
    error_type: ParseErrorType,
    description: String,
}

impl UlogParseError {
    fn new(error_type: ParseErrorType, description: &str) -> Self {
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

impl<'c> LogParser<'c> {
    pub fn set_data_message_callback<CB: FnMut(&model::DataMessage)>(&mut self, c: &'c mut CB) {
        self.data_message_callback = Some(c)
    }
    pub fn set_logged_string_message_callback<CB: FnMut(&model::LoggedStringMessage)>(
        &mut self,
        c: &'c mut CB,
    ) {
        self.logged_string_message_callback = Some(c)
    }
    pub fn consume_bytes(&mut self, mut buf: &[u8]) -> Result<(), UlogParseError> {
        if !self.leftover.is_empty() {
            assert!(self.leftover.len() < MAX_MESSAGE_SIZE);
            let original_leftover_len = self.leftover.len();
            let bytes_to_copy = std::cmp::min(buf.len(), MAX_MESSAGE_SIZE - self.leftover.len());
            self.leftover.extend_from_slice(&buf[0..bytes_to_copy]);
            // Make leftover accessible while self is borrowed immutably.
            let mut leftover = Vec::new();
            std::mem::swap(&mut leftover, &mut self.leftover);
            let leftover_bytes_used = self.parse_single_entry(leftover.as_slice())?;
            std::mem::swap(&mut leftover, &mut self.leftover);
            if leftover_bytes_used == 0 {
                // If we have no error and nothing to read within this much data, this implementation has issues.
                assert!(self.leftover.len() < MAX_MESSAGE_SIZE);
                self.leftover.truncate(original_leftover_len);
                return Ok(());
            }
            if leftover_bytes_used < original_leftover_len {
                // We are not done with the original leftovers, call this function again to get rid of that.
                self.leftover.truncate(original_leftover_len);
                self.leftover.drain(0..leftover_bytes_used);
                return self.consume_bytes(buf);
            }
            // We are done reading the leftover, start reading buf at the next unconsumed index.
            self.leftover.clear();
            buf = &buf[(leftover_bytes_used - original_leftover_len)..buf.len()];
        }
        loop {
            let num_bytes_consumed = self.parse_single_entry(&buf)?;
            if num_bytes_consumed == 0 {
                self.leftover.extend_from_slice(buf);
                return Ok(());
            }
            buf = &buf[num_bytes_consumed..];
        }
    }

    // Consumes self to make sure this is the final data_format.
    pub fn get_final_data_format(self) -> DataFormat {
        self.flattened_format
    }

    fn transition_to_data_section_if_necessary(
        &mut self,
        message_type: model::MessageType,
    ) -> Result<(), UlogParseError> {
        if !(self.status == ParseStatus::InDefinitions || self.status == ParseStatus::InData) {
            return Err(UlogParseError::new(
                ParseErrorType::Other,
                &format!("{:?} encountered in {:?}", message_type, self.status),
            ));
        }
        if self.status == ParseStatus::InDefinitions {
            self.flattened_format = DataFormat::new(flatten_format(&self.message_formats)?);
            self.status = ParseStatus::InData;
        }
        Ok(())
    }

    // Parses a header or a message.
    fn parse_single_entry(&mut self, buf: &[u8]) -> Result<usize, UlogParseError> {
        assert!(self.leftover.is_empty());
        if self.status == ParseStatus::Beginning {
            if buf.len() < 16 {
                return Ok(0);
            }
            if buf[0..7] != HEADER_BYTES {
                return Err(UlogParseError::new(
                    ParseErrorType::InvalidFile,
                    "The header does not match the template",
                ));
            }
            self.version = buf[7];
            self.timestamp = unpack::as_u64_le(&buf[8..16]);
            self.status = ParseStatus::AfterHeader;
            return Ok(16);
        }
        if buf.len() < 3 {
            return Ok(0);
        }
        let msg_size = unpack::as_u16_le(&buf[0..2]);
        let msg_type = buf[2];
        let consumed_len = msg_size as usize + 3;
        if buf.len() <= consumed_len {
            return Ok(0);
        }
        let msg = model::ULogMessage::new(msg_type, &buf[3..(3 + msg_size as usize)]);
        self.parse_message(msg)?;
        Ok(consumed_len)
    }

    fn parse_message(&mut self, msg: model::ULogMessage) -> Result<(), UlogParseError> {
        match msg.msg_type() {
            model::MessageType::FlagBits => {
                if self.status != ParseStatus::AfterHeader {
                    return Err(UlogParseError::new(
                        ParseErrorType::Other,
                        "flag bits at bad position",
                    ));
                }
                self.status = ParseStatus::InDefinitions;
                //TODO: read message
            }
            model::MessageType::Format => {
                let format = parse_format(&msg)?;
                let message_name = format.message_name.to_string();
                if self
                    .message_formats
                    .insert(format.message_name.to_string(), format.fields)
                    .is_some()
                {
                    return Err(UlogParseError::new(
                        ParseErrorType::Other,
                        &format!("duplicate message definition: {}", message_name),
                    ));
                }
            }
            model::MessageType::AddLoggedMessage => {
                self.transition_to_data_section_if_necessary(msg.msg_type())?;
                let multi_id = msg.data[0];
                let msg_id = unpack::as_u16_le(&msg.data[1..3]);
                let message_name = std::str::from_utf8(&msg.data[3..]).map_err(|_| {
                    UlogParseError::new(
                        ParseErrorType::Other,
                        &format!("format message is not a string {:?}", &msg.data[3..]),
                    )
                })?;
                self.flattened_format
                    .register_msg_id(msg_id, message_name, multi_id)?;
            }
            model::MessageType::Logging => {
                self.transition_to_data_section_if_necessary(msg.msg_type())?;
                if msg.data.len() < 9 {
                    return Err(UlogParseError::new(
                        ParseErrorType::Other,
                        &"Logged string message was too short",
                    ));
                }
                let log_level = msg.data[0];
                let timestamp = unpack::as_u64_le(&msg.data[1..9]);
                // Replace non-UTF-8 characters with placeholders, a partial message is still better than none.
                let logged_message = String::from_utf8_lossy(&msg.data[9..]);
                if let Some(cb) = &mut self.logged_string_message_callback {
                    cb(&model::LoggedStringMessage {
                        log_level,
                        timestamp,
                        logged_message: &logged_message,
                    })
                }
            }
            model::MessageType::Data => {
                if self.status != ParseStatus::InData {
                    return Err(UlogParseError::new(
                        ParseErrorType::Other,
                        "data message encountered before data section was started",
                    ));
                }

                if msg.data().len() < 2 {
                    return Err(UlogParseError::new(
                        ParseErrorType::Other,
                        "encountered data message which was too short",
                    ));
                }
                let msg_id = unpack::as_u16_le(&msg.data[0..2]);
                let (flattened_format, multi_id) = self
                    .flattened_format
                    .get_message_description(msg_id)
                    .ok_or_else(|| {
                        UlogParseError::new(
                            ParseErrorType::Other,
                            &format!("data message encountered unregistered msg_id: {}", msg_id),
                        )
                    })?;
                if flattened_format.size() != msg.size() {
                    return Err(UlogParseError::new(
                        ParseErrorType::Other,
                        &format!(
                            "data message had wrong size {}, format: {:?}",
                            msg.size(),
                            flattened_format
                        ),
                    ));
                }
                if let Some(cb) = &mut self.data_message_callback {
                    cb(&DataMessage {
                        msg_id,
                        multi_id: multi_id.clone(),
                        data: msg.data(),
                        flattened_format,
                    });
                }
            }

            _ => (),
        }
        Ok(())
    }
}

/// Log data item type
#[derive(Debug, PartialEq)]
pub enum DataType {
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
    Message(String),
}

impl DataType {
    fn from_str(written_type: &str) -> Self {
        match written_type {
            "int8_t" => DataType::Int8,
            "uint8_t" => DataType::UInt8,
            "int16_t" => DataType::Int16,
            "uint16_t" => DataType::UInt16,
            "int32_t" => DataType::Int32,
            "uint32_t" => DataType::UInt32,
            "int64_t" => DataType::Int64,
            "uint64_t" => DataType::UInt64,
            "float" => DataType::Float,
            "double" => DataType::Double,
            "bool" => DataType::Bool,
            "char" => DataType::Char,
            other => DataType::Message(other.to_string()),
        }
    }
}

#[derive(Debug)]
pub enum MaybeRepeatedType {
    Singular(DataType),
    Repeated(DataType, i16),
}

impl MaybeRepeatedType {
    fn from_str(written_type: &str) -> Result<Self, UlogParseError> {
        let split: Vec<&str> = written_type.split("[").collect();
        if split.len() == 1 {
            return Ok(MaybeRepeatedType::Singular(DataType::from_str(
                written_type,
            )));
        } else if split.len() == 2 && split[1].ends_with("]") {
            let should_be_number = &split[1][0..(split[1].len() - 1)];
            if let Ok(number) = should_be_number.parse::<i16>() {
                return Ok(MaybeRepeatedType::Repeated(
                    DataType::from_str(split[0]),
                    number,
                ));
            }
        }
        Err(UlogParseError::new(
            ParseErrorType::Other,
            &format!("invalid type string: {}", written_type),
        ))
    }
}

#[derive(Debug)]
struct Field {
    field_name: String,
    field_type: MaybeRepeatedType,
}

#[derive(Default, Debug)]
struct Format {
    message_name: String,
    fields: Vec<Field>,
}

fn parse_format(message: &model::ULogMessage) -> Result<Format, UlogParseError> {
    let format = std::str::from_utf8(&message.data()).map_err(|_| {
        UlogParseError::new(ParseErrorType::Other, "format message is not a string")
    })?;

    let parts: Vec<&str> = format.split(":").collect();

    if parts.len() != 2 || parts.iter().any(|e| e.is_empty()) {
        return Err(UlogParseError::new(
            ParseErrorType::Other,
            &format!("invalid format string: {}", format),
        ));
    }

    let mut result = Format::default();
    result.message_name = parts[0].to_string();

    for type_and_name in parts[1].split(";").filter(|s| !s.is_empty()) {
        let split: Vec<&str> = type_and_name.split(" ").collect();

        if split.len() != 2 || split.iter().any(|e| e.is_empty()) {
            return Err(UlogParseError::new(
                ParseErrorType::Other,
                &format!("invalid type_and_name string: {}", type_and_name),
            ));
        }
        let field_type = MaybeRepeatedType::from_str(split[0])?;
        let field_name = split[1].to_string();
        result.fields.push(Field {
            field_type,
            field_name,
        })
    }

    let unique_field_names =
        HashSet::<String>::from_iter(result.fields.iter().map(|f| f.field_name.to_string()));
    if unique_field_names.len() != result.fields.len() {
        return Err(UlogParseError::new(
            ParseErrorType::Other,
            &format!("duplicate field name in format string: {}", format),
        ));
    }

    Ok(result)
}

fn flatten_data_type(
    data_type: &DataType,
    qualified_field_name: String,
    mut offset: usize,
    message_formats: &HashMap<String, Vec<Field>>,
    already_added_messages: &mut HashSet<String>,
    list_to_append_to: &mut Vec<FlattenedField>,
) -> Result<usize, UlogParseError> {
    match data_type {
        DataType::Int8 => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::Int8,
                offset: offset as u16,
            });
            offset += 1;
        }
        DataType::UInt8 => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::UInt8,
                offset: offset as u16,
            });
            offset += 1;
        }
        DataType::Int16 => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::Int16,
                offset: offset as u16,
            });
            offset += 2;
        }
        DataType::UInt16 => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::UInt16,
                offset: offset as u16,
            });
            offset += 2;
        }
        DataType::Int32 => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::Int32,
                offset: offset as u16,
            });
            offset += 4;
        }
        DataType::UInt32 => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::UInt32,
                offset: offset as u16,
            });
            offset += 4;
        }
        DataType::Int64 => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::Int64,
                offset: offset as u16,
            });
            offset += 8;
        }
        DataType::UInt64 => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::UInt64,
                offset: offset as u16,
            });
            offset += 8;
        }
        DataType::Float => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::Float,
                offset: offset as u16,
            });
            offset += 4;
        }
        DataType::Double => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::Double,
                offset: offset as u16,
            });
            offset += 8;
        }
        DataType::Bool => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::Bool,
                offset: offset as u16,
            });
            offset += 1;
        }
        DataType::Char => {
            list_to_append_to.push(FlattenedField {
                flattened_field_name: qualified_field_name,
                field_type: FlattenedFieldType::Char,
                offset: offset as u16,
            });
            offset += 1;
        }
        DataType::Message(message_name) => {
            offset = add_flattened_message(
                message_name,
                offset,
                message_formats,
                qualified_field_name + message_name + ".",
                already_added_messages,
                list_to_append_to,
            )?;
            already_added_messages.remove(message_name);
        }
    }
    let u16_offset = offset as u16;
    if u16_offset as usize != offset {
        return Err(UlogParseError::new(
            ParseErrorType::Other,
            "offset overflow",
        ));
    }
    Ok(offset)
}

fn flatten_field(
    field: &Field,
    mut offset: usize,
    message_formats: &HashMap<String, Vec<Field>>,
    hierarchical_message_prefix: String,
    already_added_messages: &mut HashSet<String>,
    list_to_append_to: &mut Vec<FlattenedField>,
) -> Result<usize, UlogParseError> {
    match &field.field_type {
        MaybeRepeatedType::Repeated(dt, n) => {
            for i in 0..*n {
                offset = flatten_data_type(
                    dt,
                    hierarchical_message_prefix.to_string()
                        + &field.field_name
                        + &format!("[{}]", i),
                    offset,
                    message_formats,
                    already_added_messages,
                    list_to_append_to,
                )?;
            }
        }
        MaybeRepeatedType::Singular(dt) => {
            offset = flatten_data_type(
                dt,
                hierarchical_message_prefix + &field.field_name,
                offset,
                message_formats,
                already_added_messages,
                list_to_append_to,
            )?;
        }
    }
    Ok(offset)
}

fn add_flattened_message(
    message_name: &String,
    mut offset: usize,
    message_formats: &HashMap<String, Vec<Field>>,
    hierarchical_message_prefix: String,
    already_added_messages: &mut HashSet<String>,
    list_to_append_to: &mut Vec<FlattenedField>,
) -> Result<usize, UlogParseError> {
    if !already_added_messages.insert(message_name.to_string()) {
        return Err(UlogParseError::new(
            ParseErrorType::Other,
            &format!("Found circular reference to {}", message_name),
        ));
    }

    let mut padding_trash_vec = Vec::new();
    if let Some(fields) = message_formats.get(message_name) {
        for field in fields {
            if hierarchical_message_prefix.is_empty()
                && field.field_name.starts_with("_padding")
                && field.field_name == fields.last().unwrap().field_name
            {
                // padding is skipped on the last field on the base level
                break;
            }
            let append_to;
            // Only add the name for non-padding fields
            if field.field_name.starts_with("_padding") {
                append_to = &mut padding_trash_vec;
            } else {
                append_to = list_to_append_to;
            }
            offset = flatten_field(
                field,
                offset,
                message_formats,
                hierarchical_message_prefix.to_string(),
                already_added_messages,
                append_to,
            )?;
        }
        Ok(offset)
    } else {
        Err(UlogParseError::new(
            ParseErrorType::Other,
            &format!(
                "Could not find format definition for message {}",
                message_name
            ),
        ))
    }
}

fn flatten_format(
    message_formats: &HashMap<String, Vec<Field>>,
) -> Result<HashMap<String, FlattenedFormat>, UlogParseError> {
    // for each message_format:
    //   hashset to keep track of used messages (always initialized with the name of the expanding message)
    //   use recursive helper function to expand stuff, arguments: (offset, field_prefix = "", message_formats, mut list_to_append_to) -> offset
    //     which skips writing "_padding" fields at the end of the message if the prefix is empty.

    let mut result = HashMap::new();
    for field in message_formats {
        let mut already_added_messages = HashSet::<String>::new();
        let mut offset = 2; // for the msg_id
        let mut flattened_fields = Vec::<FlattenedField>::new();
        let message_name = field.0;
        offset = add_flattened_message(
            message_name,
            offset,
            message_formats,
            "".to_string(),
            &mut already_added_messages,
            &mut flattened_fields,
        )?;
        let u16_offset = offset as u16;
        if u16_offset as usize != offset {
            return Err(UlogParseError::new(
                ParseErrorType::Other,
                &format!("Message is too big {}", message_name),
            ));
        }
        result.insert(
            message_name.to_string(),
            FlattenedFormat::new(message_name.to_string(), flattened_fields, u16_offset),
        );
    }

    Ok(result)
}

pub enum SimpleCallbackResult {
    KeepReading,
    Stop,
}

pub enum Message<'a> {
    Data(&'a model::DataMessage<'a>),
    LoggedMessage(&'a model::LoggedStringMessage<'a>),
}

pub fn read_file_with_simple_callback<CB: FnMut(&Message) -> SimpleCallbackResult>(
    file_path: &str,
    c: &mut CB,
) -> Result<usize, std::io::Error> {
    let stop_reading = Arc::new(AtomicBool::new(false));
    let c_cell: Rc<RefCell<&mut CB>> = Rc::new(RefCell::new(c));
    let mut wrapped_data_message_callback = |data_message: &DataMessage| {
        if let SimpleCallbackResult::Stop =
            c_cell.as_ref().borrow_mut().deref_mut()(&Message::Data(&data_message))
        {
            stop_reading.store(true, Ordering::Relaxed)
        }
    };
    let mut wrapped_string_message_callback = |data_message: &model::LoggedStringMessage| {
        if let SimpleCallbackResult::Stop =
            c_cell.as_ref().borrow_mut().deref_mut()(&Message::LoggedMessage(&data_message))
        {
            stop_reading.store(true, Ordering::Relaxed)
        }
    };
    let mut log_parser = LogParser::default();
    log_parser.set_data_message_callback(&mut wrapped_data_message_callback);
    log_parser.set_logged_string_message_callback(&mut wrapped_string_message_callback);

    let mut total_bytes_read: usize = 0;
    let mut f = std::fs::File::open(file_path)?;
    const READ_START: usize = 64 * 1024;
    let mut buf = [0u8; 1024 * 1024];
    while !stop_reading.load(Ordering::Relaxed) {
        let num_bytes_read = f.read(&mut buf[READ_START..])?;
        if num_bytes_read == 0 {
            break;
        }
        log_parser
            .consume_bytes(&buf[READ_START..(READ_START + num_bytes_read)])
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, format!("err: {:?}", e)))?;
        total_bytes_read += num_bytes_read;
    }
    Ok(total_bytes_read)
}
