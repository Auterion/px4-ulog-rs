//! Tests for timestamp field handling.
//!
//! Per the ULog specification the timestamp field must be uint64_t microseconds.
//! Earlier spec revisions allowed truncated uint8/uint16/uint32 timestamps;
//! those were removed as unused, so the parser only recognizes a uint64 field
//! named "timestamp" as the timestamp. A field named "timestamp" of any other
//! type is treated as an ordinary field, not the timestamp.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::LogParser;
use px4_ulog::stream_parser::model::DataMessage;

/// Build a one-message log whose `timestamp` field has the given type, parse it,
/// and return (had_timestamp_field, parsed_timestamp_if_any) from the first data
/// message.
fn parse_with_timestamp_type(ts_type_str: &str, ts_bytes: &[u8]) -> (bool, Option<u64>) {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("sensor_data", &[(ts_type_str, "timestamp"), ("float", "value")])
        .add_logged(0, 0, "sensor_data");

    let mut field_data = Vec::new();
    field_data.extend_from_slice(ts_bytes);
    field_data.extend_from_slice(&1.5f32.to_le_bytes());
    builder.data(0, &field_data);

    let bytes = builder.build();

    let mut had_field = false;
    let mut value = None;
    let mut cb = |msg: &DataMessage| {
        if let Some(ts) = msg.flattened_format.timestamp_field.as_ref() {
            had_field = true;
            value = Some(ts.parse_timestamp(msg.data));
        }
    };

    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut cb);
    parser.consume_bytes(&bytes).expect("parse should succeed");

    (had_field, value)
}

#[test]
fn uint64_timestamp_is_parsed() {
    let ts_val: u64 = 123_456_789;
    let (had_field, parsed) = parse_with_timestamp_type("uint64_t", &ts_val.to_le_bytes());
    assert!(had_field, "uint64 timestamp should be recognized");
    assert_eq!(parsed, Some(ts_val));
}

#[test]
fn uint64_timestamp_value_is_preserved() {
    let ts_val: u64 = 1_000_000_000;
    let (_, parsed) = parse_with_timestamp_type("uint64_t", &ts_val.to_le_bytes());
    assert_eq!(parsed, Some(1_000_000_000));
}

#[test]
fn non_uint64_timestamp_field_is_ignored() {
    // A "timestamp" field that is not uint64 is not treated as the timestamp.
    let (had_u32, _) = parse_with_timestamp_type("uint32_t", &1_000_000u32.to_le_bytes());
    assert!(!had_u32, "uint32 timestamp field should be ignored");

    let (had_u16, _) = parse_with_timestamp_type("uint16_t", &50_000u16.to_le_bytes());
    assert!(!had_u16, "uint16 timestamp field should be ignored");

    let (had_u8, _) = parse_with_timestamp_type("uint8_t", &[200u8]);
    assert!(!had_u8, "uint8 timestamp field should be ignored");
}
