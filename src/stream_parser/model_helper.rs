use super::model::FlattenedFieldType;
use byteorder::ByteOrder;

pub trait LittleEndianParser {
    fn parse(serialized: &[u8]) -> Self;
}
impl LittleEndianParser for i8 {
    fn parse(serialized: &[u8]) -> Self {
        serialized[0] as i8
    }
}
impl LittleEndianParser for u8 {
    fn parse(serialized: &[u8]) -> Self {
        serialized[0]
    }
}
impl LittleEndianParser for i16 {
    fn parse(serialized: &[u8]) -> Self {
        byteorder::LittleEndian::read_i16(serialized)
    }
}
impl LittleEndianParser for u16 {
    fn parse(serialized: &[u8]) -> Self {
        byteorder::LittleEndian::read_u16(serialized)
    }
}
impl LittleEndianParser for i32 {
    fn parse(serialized: &[u8]) -> Self {
        byteorder::LittleEndian::read_i32(serialized)
    }
}
impl LittleEndianParser for u32 {
    fn parse(serialized: &[u8]) -> Self {
        byteorder::LittleEndian::read_u32(serialized)
    }
}
impl LittleEndianParser for i64 {
    fn parse(serialized: &[u8]) -> Self {
        byteorder::LittleEndian::read_i64(serialized)
    }
}
impl LittleEndianParser for u64 {
    fn parse(serialized: &[u8]) -> Self {
        byteorder::LittleEndian::read_u64(serialized)
    }
}
impl LittleEndianParser for f32 {
    fn parse(serialized: &[u8]) -> Self {
        byteorder::LittleEndian::read_f32(serialized)
    }
}
impl LittleEndianParser for f64 {
    fn parse(serialized: &[u8]) -> Self {
        byteorder::LittleEndian::read_f64(serialized)
    }
}
impl LittleEndianParser for char {
    fn parse(serialized: &[u8]) -> Self {
        serialized[0] as char
    }
}
impl LittleEndianParser for bool {
    fn parse(serialized: &[u8]) -> Self {
        serialized[0] != 0
    }
}

pub trait FlattenedFieldTypeMatcher {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool;
}
impl FlattenedFieldTypeMatcher for i8 {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::Int8
    }
}
impl FlattenedFieldTypeMatcher for u8 {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::UInt8
    }
}
impl FlattenedFieldTypeMatcher for i16 {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::Int16
    }
}
impl FlattenedFieldTypeMatcher for u16 {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::UInt16
    }
}
impl FlattenedFieldTypeMatcher for i32 {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::Int32
    }
}
impl FlattenedFieldTypeMatcher for u32 {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::UInt32
    }
}
impl FlattenedFieldTypeMatcher for i64 {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::Int64
    }
}
impl FlattenedFieldTypeMatcher for u64 {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::UInt64
    }
}
impl FlattenedFieldTypeMatcher for f32 {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::Float
    }
}
impl FlattenedFieldTypeMatcher for f64 {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::Double
    }
}
impl FlattenedFieldTypeMatcher for char {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::Char
    }
}
impl FlattenedFieldTypeMatcher for bool {
    fn matches(flat_field_type: &FlattenedFieldType) -> bool {
        *flat_field_type == FlattenedFieldType::Bool
    }
}
