use super::model_helper::{FlattenedFieldTypeMatcher, LittleEndianParser};
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;

/// The kind of a ULog message, decoded from its one-byte type tag.
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
    TaggedLogging,
    ParameterDefault,
    FlagBits,
}

/// A single raw ULog message: its type tag plus the payload bytes.
pub struct ULogMessage<'a> {
    msg_type: u8,
    pub data: &'a [u8],
}

impl<'a> ULogMessage<'a> {
    // Returns the # bytes consumed
    //pub fn parse(data: &'a [u8]) -> (Option<Self>, usize) {}

    pub fn new(msg_type: u8, data: &'a [u8]) -> Self {
        if data.len() > u16::MAX as usize {
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
            'C' => MessageType::TaggedLogging,
            'Q' => MessageType::ParameterDefault,
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

/// The scalar type of a flattened message field.
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

/// A single decoded field value, tagged with its scalar type.
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

/// Instance index distinguishing multiple subscriptions to the same topic.
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

/// One field of a flattened message format: its name, type, and byte offset
/// within the message payload.
#[derive(Clone, Debug)]
pub struct FlattenedField {
    pub flattened_field_name: String,
    pub field_type: FlattenedFieldType,
    pub offset: u16, // relative to the beginning of the message ()
}

/// The uint64 microsecond timestamp field of a message, located by byte offset.
#[derive(Clone, Debug)]
pub struct TimestampField {
    pub offset: u16, // relative to the beginning of the message
}

impl TimestampField {
    pub fn parse_timestamp(&self, data: &[u8]) -> u64 {
        // The ULog spec requires the timestamp field to be uint64 microseconds.
        u64::parse(&data[self.offset as usize..])
    }
}

/// Why a field lookup failed: the field is absent, or present with another type.
#[derive(Debug)]
pub enum FieldLookupError {
    MissingField,
    TypeMismatch,
}

/// An error encountered while parsing a ULog stream.
#[derive(Debug)]
pub struct UlogParseError {
    pub error_type: ParseErrorType,
    pub description: String,
}

impl UlogParseError {
    pub fn new(error_type: ParseErrorType, description: &str) -> Self {
        Self {
            error_type,
            description: description.to_string(),
        }
    }
}

/// Broad category of a [`UlogParseError`].
#[derive(Debug)]
pub enum ParseErrorType {
    InvalidFile,
    Other,
}

impl fmt::Display for ParseErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseErrorType::InvalidFile => write!(f, "invalid file"),
            ParseErrorType::Other => write!(f, "parse error"),
        }
    }
}

impl fmt::Display for UlogParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.error_type, self.description)
    }
}

impl std::error::Error for UlogParseError {}

/// A message format flattened into its concrete fields, with offsets resolved
/// and the timestamp field (if any) identified. Used to decode Data messages.
#[derive(Clone, Debug)]
pub struct FlattenedFormat {
    pub message_name: String,
    pub fields: Vec<FlattenedField>,
    name_to_field: HashMap<String, FlattenedField>,
    pub timestamp_field: Option<TimestampField>,
    size: u16,
}

/// A scalar type that can be decoded from a field and matched against a
/// [`FlattenedFieldType`].
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
        // Per the ULog spec the timestamp field is uint64 microseconds; a field
        // named "timestamp" of any other type is not treated as the timestamp.
        let timestamp_field = name_to_field
            .get("timestamp")
            .filter(|field| field.field_type == FlattenedFieldType::UInt64)
            .map(|field| TimestampField {
                offset: field.offset,
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

    pub fn field_iter(&self) -> std::slice::Iter<'_, FlattenedField> {
        self.fields.iter()
    }

    pub fn message_name(&self) -> &str {
        &self.message_name
    }

    pub fn size(&self) -> u16 {
        self.size
    }
}

/// A reusable decoder for one typed field at a fixed offset within a message.
pub struct FieldParser<T: ParseableFieldType> {
    offset: u16, // relative to the beginning of the message ()
    _phantom: PhantomData<T>,
}

impl<T: ParseableFieldType> FieldParser<T> {
    // data e.g. looks like the member in the DataMessage
    pub fn parse(&self, data: &[u8]) -> T {
        T::parse(&data[(self.offset as usize)..])
    }
    pub fn offset(&self) -> u16 {
        self.offset
    }
}

/// A logged data sample: the subscription it belongs to, its format, and the
/// raw payload (including the leading msg_id bytes).
pub struct DataMessage<'a> {
    pub msg_id: u16,
    pub multi_id: MultiId,
    pub flattened_format: &'a FlattenedFormat,
    pub data: &'a [u8], // this includes the bytes of the msg_id.
}

/// Which section of the log a message appeared in: the definitions header or
/// the data section.
#[derive(Clone, Debug, PartialEq)]
pub enum LogStage {
    Definitions,
    Data,
}

/// A parameter value, tagged with the section it appeared in (initial value in
/// definitions, or a mid-log change in the data section).
#[derive(Debug)]
pub enum ParameterMessage<'a> {
    Float(&'a str, f32, LogStage),
    Int32(&'a str, i32, LogStage),
}

/// A human-readable log string with its severity level and timestamp.
pub struct LoggedStringMessage<'a> {
    pub log_level: u8,
    pub timestamp: u64,
    pub logged_message: &'a str,
}

/// A key/value information message. The key in the file is a typed declaration
/// `"type name"` (e.g. `char[10] sys_name`); `type_str` is the type portion and
/// `key` the name. Consumers use `type_str` to interpret the raw `value` bytes.
pub struct InfoMessage<'a> {
    pub type_str: &'a str,
    pub key: &'a str,
    pub value: &'a [u8],
}

/// A logging dropout: a gap of `duration_ms` milliseconds where data was lost.
pub struct DropoutMessage {
    pub duration_ms: u16,
}

/// A synchronization marker used to resynchronize after corruption.
pub struct SyncMessage {
    pub magic: [u8; 8],
}

/// One fragment of a multi-value information message. Like [`InfoMessage`], the
/// key is a typed declaration; `type_str` is the type portion and `key` the name.
/// `is_continued` is true when more fragments for this key follow.
pub struct MultiInfoMessage<'a> {
    pub is_continued: bool,
    pub type_str: &'a str,
    pub key: &'a str,
    pub value: &'a [u8],
}

/// A reassembled multi-info message whose fragments have been concatenated.
/// Owns its data since the value is built from multiple message payloads.
#[derive(Clone, Debug)]
pub struct ReassembledMultiInfoMessage {
    pub key: String,
    pub value: Vec<u8>,
}

/// Marks a previously-subscribed message id as removed (no further data).
pub struct RemoveLoggedMessage {
    pub msg_id: u16,
}

/// A logged string carrying an extra `tag` identifying its source/category.
pub struct TaggedLoggedStringMessage<'a> {
    pub log_level: u8,
    pub tag: u16,
    pub timestamp: u64,
    pub logged_message: &'a str,
}

/// A parameter's default value, with `default_types` flags indicating which
/// default scopes (system / current setup) it applies to.
#[derive(Debug)]
pub enum ParameterDefaultMessage<'a> {
    Float(&'a str, f32, u8),
    Int32(&'a str, i32, u8),
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
