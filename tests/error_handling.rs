//! Error handling tests. Verifies UlogParseError implements the standard error
//! traits and that every distinct parse-failure mode returns `Err` with a
//! descriptive message rather than silently succeeding.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::LogParser;
use px4_ulog::stream_parser::model::{DataMessage, ParseErrorType, UlogParseError};

fn parse_bytes(bytes: &[u8]) -> Result<(), UlogParseError> {
    let mut noop_data = |_: &DataMessage| {};
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut noop_data);
    parser.consume_bytes(bytes)
}

// =============================================================================
// Error trait contract
// =============================================================================

#[test]
fn error_is_boxable_as_std_error_with_display() {
    let err = UlogParseError::new(ParseErrorType::InvalidFile, "bad header");
    let boxed: Box<dyn std::error::Error> = Box::new(err);
    assert!(!format!("{}", boxed).is_empty());
}

#[test]
fn debug_and_display_carry_context() {
    let err = UlogParseError::new(ParseErrorType::InvalidFile, "bad header");
    let debug = format!("{:?}", err);
    let display = format!("{}", err);
    assert!(
        debug.contains("UlogParseError"),
        "Debug should name the type, got: {}",
        debug
    );
    assert!(
        display.contains("bad header"),
        "Display should carry description, got: {}",
        display
    );
}

// =============================================================================
// Distinct parse-failure modes
// =============================================================================
//
// Each test exercises one failure cause and asserts both that the parser returns
// Err and that the error message names the offending element. Testing both in
// one place avoids duplication.

#[test]
fn invalid_header_magic_is_rejected() {
    let mut bytes = vec![0xDE, 0xAD, 0xBE, 0xEF, 0, 0, 0, 1];
    bytes.extend_from_slice(&0u64.to_le_bytes());
    assert_eq!(bytes.len(), 16);
    let err = parse_bytes(&bytes).unwrap_err();
    assert!(format!("{:?}", err).contains("InvalidFile"));
}

#[test]
fn unknown_incompat_flag_bit_is_rejected() {
    let mut builder = ULogBuilder::new();
    let mut incompat = [0u8; 8];
    incompat[0] = 0x02; // bit 1 is unknown
    builder.flag_bits_with_incompat(&incompat);
    let err = parse_bytes(&builder.build()).unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("incompatible") || msg.contains("flag"),
        "got: {}",
        msg
    );
}

#[test]
fn duplicate_format_name_is_rejected() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("accel", &[("uint64_t", "timestamp"), ("float", "x")])
        .format("accel", &[("uint64_t", "timestamp"), ("float", "y")])
        .add_logged(0, 0, "accel");
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u64.to_le_bytes());
    payload.extend_from_slice(&1.0f32.to_le_bytes());
    builder.data(0, &payload);

    let err = parse_bytes(&builder.build()).unwrap_err();
    let msg = format!("{:?}", err);
    assert!(msg.to_lowercase().contains("duplicate"), "got: {}", msg);
    assert!(
        msg.contains("accel"),
        "should name offending format, got: {}",
        msg
    );
}

#[test]
fn duplicate_msg_id_registration_is_rejected() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("accel", &[("uint64_t", "timestamp")])
        .add_logged(7, 0, "accel")
        .add_logged(7, 1, "accel");
    builder.data(7, &0u64.to_le_bytes());

    let err = parse_bytes(&builder.build()).unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("7") || msg.to_lowercase().contains("duplicate"),
        "got: {}",
        msg
    );
}

#[test]
fn data_for_unregistered_msg_id_is_rejected() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("gyro", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "gyro");
    builder.data(42, &0u64.to_le_bytes());

    let err = parse_bytes(&builder.build()).unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("42") || msg.contains("unregistered"),
        "got: {}",
        msg
    );
}

#[test]
fn data_message_size_mismatch_is_rejected() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("baro", &[("uint64_t", "timestamp"), ("float", "pressure")])
        .add_logged(0, 0, "baro");
    builder.data(0, &[0u8; 2]); // expected 12, got 2

    let err = parse_bytes(&builder.build()).unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.to_lowercase().contains("size") || msg.contains("wrong"),
        "got: {}",
        msg
    );
}

#[test]
fn subscription_for_undefined_format_is_rejected() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("real_topic", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "nonexistent_topic");
    builder.data(0, &0u64.to_le_bytes());

    let err = parse_bytes(&builder.build()).unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("nonexistent_topic") || msg.contains("format definition"),
        "got: {}",
        msg
    );
}

#[test]
fn malformed_format_strings_are_rejected() {
    // Covers the three distinct malformations in one parameterised check:
    // missing colon separator, empty type name, missing space between type and field.
    let cases: &[&[u8]] = &[
        b"no_colon_here",             // missing colon
        b":just_a_colon_no_name",     // empty name before colon
        b"topic_x:uint64_ttimestamp", // no space between type and field name
    ];
    for bad in cases {
        let mut bytes = ULogBuilder::new().flag_bits().build();
        bytes.extend_from_slice(&(bad.len() as u16).to_le_bytes());
        bytes.push(b'F');
        bytes.extend_from_slice(bad);
        assert!(
            parse_bytes(&bytes).is_err(),
            "malformed format {:?} should be rejected",
            std::str::from_utf8(bad).unwrap()
        );
    }
}

#[test]
fn duplicate_field_names_in_format_is_rejected() {
    let builder = ULogBuilder::new();
    let format_str = b"dup_fields:uint64_t timestamp;float timestamp";
    let mut bytes = builder.build();
    bytes.extend_from_slice(&(format_str.len() as u16).to_le_bytes());
    bytes.push(b'F');
    bytes.extend_from_slice(format_str);

    let err = parse_bytes(&bytes).unwrap_err();
    let msg = format!("{:?}", err);
    assert!(
        msg.to_lowercase().contains("duplicate") || msg.contains("field"),
        "got: {}",
        msg
    );
}

// =============================================================================
// File-based APIs propagate errors instead of silently succeeding
// =============================================================================

#[test]
fn read_file_with_simple_callback_returns_error_for_bad_file() {
    use px4_ulog::stream_parser::file_reader::{
        read_file_with_simple_callback, Message, SimpleCallbackResult,
    };
    use std::io::Write;

    let path = std::env::temp_dir().join("px4_ulog_error_handling_simple_bad.ulg");
    std::fs::File::create(&path)
        .unwrap()
        .write_all(&[0xFF; 16])
        .unwrap();

    let mut noop = |_: &Message| SimpleCallbackResult::KeepReading;
    let result = read_file_with_simple_callback(path.to_str().unwrap(), &mut noop);
    assert!(result.is_err());

    std::fs::remove_file(&path).ok();
}

#[test]
fn full_parser_read_file_returns_error_for_bad_file() {
    use std::io::Write;

    let path = std::env::temp_dir().join("px4_ulog_error_handling_full_bad.ulg");
    std::fs::File::create(&path)
        .unwrap()
        .write_all(&[0xFF; 16])
        .unwrap();

    assert!(px4_ulog::full_parser::read_file(path.to_str().unwrap()).is_err());

    std::fs::remove_file(&path).ok();
}
