//! Test-driven development tests for proper error handling in px4-ulog-rs.
//!
//! These tests define what error handling SHOULD look like. Many will fail until
//! the source code is updated to match. That is intentional -- this is TDD.
//!
//! Current problems this file exposes:
//! 1. UlogParseError does not implement std::error::Error or std::fmt::Display
//! 2. UlogParseError fields (error_type, description) are private and never read
//! 3. Only two error variants: InvalidFile and Other (Other is a catch-all)
//! 4. The seek parser (dataset.rs:82-86) silently converts all errors to None

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::LogParser;
use px4_ulog::stream_parser::model::{DataMessage, ParseErrorType, UlogParseError};

/// Feed bytes through a LogParser and return the result.
fn parse_bytes(bytes: &[u8]) -> Result<(), UlogParseError> {
    let mut noop_data = |_: &DataMessage| {};
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut noop_data);
    parser.consume_bytes(bytes)
}

// =============================================================================
// 1. UlogParseError should implement std::error::Error
// =============================================================================
//
// Currently UlogParseError does NOT implement std::error::Error or Display.
// These tests will fail to compile until that impl is added.
//
// TODO: Uncomment once UlogParseError implements std::error::Error and Display.

#[test]
fn error_implements_std_error_trait() {
    // UlogParseError should be usable as Box<dyn std::error::Error>.
    let err = UlogParseError::new(ParseErrorType::InvalidFile, "bad header");
    let boxed: Box<dyn std::error::Error> = Box::new(err);
    // The error trait requires Display, so this should work:
    let msg = format!("{}", boxed);
    assert!(!msg.is_empty(), "Display output should not be empty");
}

#[test]
fn error_works_with_question_mark_operator() {
    // Simulates using ? in a function returning Box<dyn std::error::Error>.
    fn inner() -> Result<(), Box<dyn std::error::Error>> {
        let bytes = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00, 0x00,
                         0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let mut noop = |_: &DataMessage| {};
        let mut parser = LogParser::default();
        parser.set_data_message_callback(&mut noop);
        parser.consume_bytes(&bytes)?;
        Ok(())
    }
    let result = inner();
    assert!(result.is_err(), "Invalid header should produce an error");
}

// =============================================================================
// 2. UlogParseError should implement Display with meaningful messages
// =============================================================================
//
// TODO: Uncomment once UlogParseError implements Display.

#[test]
fn display_for_invalid_header_contains_context() {
    let err = UlogParseError::new(ParseErrorType::InvalidFile, "The header does not match");
    let msg = format!("{}", err);
    assert!(msg.contains("header") || msg.contains("invalid") || msg.contains("Header"),
            "Display should mention the header problem, got: {}", msg);
}

#[test]
fn display_for_other_error_contains_description() {
    let err = UlogParseError::new(ParseErrorType::Other, "duplicate registration for msg_id 42");
    let msg = format!("{}", err);
    assert!(msg.contains("42"),
            "Display should include the description context, got: {}", msg);
}

// =============================================================================
// 3. UlogParseError implements Debug (already derived, just verify)
// =============================================================================

#[test]
fn error_implements_debug() {
    let err = UlogParseError::new(ParseErrorType::InvalidFile, "test description");
    let debug_str = format!("{:?}", err);
    assert!(!debug_str.is_empty(), "Debug output should not be empty");
    // The derived Debug should include the struct/field names.
    assert!(
        debug_str.contains("UlogParseError"),
        "Debug should contain the type name, got: {}",
        debug_str
    );
}

// =============================================================================
// 4. Specific error variants -- testing that different failure modes produce
//    distinguishable errors. Currently everything is ParseErrorType::Other.
// =============================================================================
//
// Tests below use the CURRENT API (ParseErrorType::InvalidFile / Other) and
// verify that errors are raised. Comments indicate what the variant SHOULD be
// once the error enum is expanded.

#[test]
fn invalid_header_wrong_magic_bytes() {
    // Feed 16 bytes with wrong magic -- should error with InvalidFile.
    // FUTURE: Should be ParseErrorType::InvalidHeader
    let mut bytes = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x00, 0x00];
    bytes.push(0x01); // version
    bytes.extend_from_slice(&0u64.to_le_bytes()); // timestamp
    assert_eq!(bytes.len(), 16);

    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "Wrong magic bytes must produce an error");
    let err = result.unwrap_err();
    let debug = format!("{:?}", err);
    assert!(
        debug.contains("InvalidFile"),
        "Wrong magic should be InvalidFile, got: {}",
        debug
    );
}

#[test]
fn incompatible_flag_bits_are_rejected() {
    // Set an unknown incompat flag bit (bit 1 of byte 0, beyond DATA_APPENDED).
    // FUTURE: Should be ParseErrorType::InvalidFlagBits
    let mut builder = ULogBuilder::new();
    let mut incompat = [0u8; 8];
    incompat[0] = 0x02; // bit 1 set, which is unknown
    builder.flag_bits_with_incompat(&incompat);
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Incompatible flag bits must produce an error"
    );
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("incompatible") || debug.contains("flag"),
        "Error should mention incompatible flags, got: {}",
        debug
    );
}

#[test]
fn incompatible_flag_bits_higher_byte_rejected() {
    // Set an unknown incompat flag in byte index 2.
    // FUTURE: Should be ParseErrorType::InvalidFlagBits
    let mut builder = ULogBuilder::new();
    let mut incompat = [0u8; 8];
    incompat[2] = 0xFF;
    builder.flag_bits_with_incompat(&incompat);
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Incompatible flags in higher bytes must produce an error"
    );
}

#[test]
fn duplicate_format_definition_is_error() {
    // Register the same format name twice.
    // FUTURE: Should be ParseErrorType::DuplicateFormat
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("sensor_accel", &[("uint64_t", "timestamp"), ("float", "x")])
        .format("sensor_accel", &[("uint64_t", "timestamp"), ("float", "y")]);
    // Need to trigger transition to data section to flatten formats.
    builder.add_logged(0, 0, "sensor_accel");
    // Add a data message to trigger data section.
    let mut data_payload = Vec::new();
    data_payload.extend_from_slice(&0u64.to_le_bytes());
    data_payload.extend_from_slice(&1.0f32.to_le_bytes());
    builder.data(0, &data_payload);
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Duplicate format definition must produce an error"
    );
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("duplicate") || debug.contains("Duplicate"),
        "Error should mention duplicate, got: {}",
        debug
    );
}

#[test]
fn duplicate_subscription_same_msg_id_is_error() {
    // Register the same msg_id twice via AddLoggedMessage.
    // FUTURE: Should be ParseErrorType::DuplicateSubscription
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("topic_a", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "topic_a")
        .add_logged(0, 1, "topic_a"); // same msg_id=0, different multi_id
    // Trigger data parsing with a data message.
    builder.data(0, &0u64.to_le_bytes());
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Duplicate msg_id registration must produce an error"
    );
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("duplicate") || debug.contains("msg_id"),
        "Error should mention duplicate registration, got: {}",
        debug
    );
}

#[test]
fn data_for_unregistered_msg_id_is_error() {
    // Send a data message for a msg_id that was never registered.
    // FUTURE: Should be ParseErrorType::UnknownSubscription
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("topic_a", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "topic_a");
    // Send data with msg_id=99, which was never registered.
    builder.data(99, &0u64.to_le_bytes());
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Data for unregistered msg_id must produce an error"
    );
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("unregistered") || debug.contains("99"),
        "Error should mention unregistered msg_id, got: {}",
        debug
    );
}

#[test]
fn data_message_wrong_size_is_error() {
    // Format says uint64_t timestamp (8 bytes) + float x (4 bytes) = 14 bytes total
    // (including 2-byte msg_id prefix). Send a data message with wrong size.
    // FUTURE: Should be ParseErrorType::MessageSizeMismatch
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("test_msg", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "test_msg");
    // Correct data size: 2 (msg_id) + 8 (timestamp) + 4 (float) = 14 bytes in the message.
    // The format size is 14 (2 for msg_id offset + 8 + 4).
    // Send only 2 + 4 = 6 bytes (too short).
    builder.data(0, &[0u8; 4]); // 2 (msg_id from .data()) + 4 = 6 byte payload
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Data message with wrong size must produce an error"
    );
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("wrong size") || debug.contains("size"),
        "Error should mention size mismatch, got: {}",
        debug
    );
}

#[test]
fn invalid_format_string_no_colon() {
    // A format message without the required ':' separator is invalid.
    // FUTURE: Should be ParseErrorType::InvalidFormatString
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    // Manually write a format message with invalid content (no colon).
    let invalid_format = b"this_has_no_colon_separator";
    let msg_size = invalid_format.len() as u16;
    let mut bytes = builder.build();
    bytes.extend_from_slice(&msg_size.to_le_bytes());
    bytes.push(b'F');
    bytes.extend_from_slice(invalid_format);

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Format string without colon must produce an error"
    );
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("invalid") || debug.contains("format"),
        "Error should mention invalid format string, got: {}",
        debug
    );
}

#[test]
fn invalid_format_string_empty_name() {
    // A format message with empty name (starts with ':') is invalid.
    // FUTURE: Should be ParseErrorType::InvalidFormatString
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    let invalid_format = b":uint64_t timestamp";
    let msg_size = invalid_format.len() as u16;
    let mut bytes = builder.build();
    bytes.extend_from_slice(&msg_size.to_le_bytes());
    bytes.push(b'F');
    bytes.extend_from_slice(invalid_format);

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Format string with empty name must produce an error"
    );
}

#[test]
fn invalid_format_string_bad_field_definition() {
    // A format field without a space between type and name.
    // FUTURE: Should be ParseErrorType::InvalidFormatString
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    // "topic_x:uint64_ttimestamp" -- no space
    let invalid_format = b"topic_x:uint64_ttimestamp";
    let msg_size = invalid_format.len() as u16;
    let mut bytes = builder.build();
    bytes.extend_from_slice(&msg_size.to_le_bytes());
    bytes.push(b'F');
    bytes.extend_from_slice(invalid_format);

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Format field without space separator must produce an error"
    );
}

// =============================================================================
// 4b. Tests for error variants that CANNOT compile against the current API
//     because the variants don't exist yet.
// =============================================================================
//
// TODO: Uncomment once ParseErrorType has these variants:
//   InvalidHeader, InvalidFlagBits, DuplicateFormat, DuplicateSubscription,
//   UnknownSubscription, MessageSizeMismatch, InvalidFormatString, TruncatedMessage

// #[test]
// fn error_variant_invalid_header() {
//     let bytes = vec![0xFF; 16];
//     let err = parse_bytes(&bytes).unwrap_err();
//     assert!(matches!(err.error_type(), ParseErrorType::InvalidHeader),
//             "Wrong magic should produce InvalidHeader");
// }

// #[test]
// fn error_variant_invalid_flag_bits() {
//     let mut builder = ULogBuilder::new();
//     let mut incompat = [0u8; 8];
//     incompat[0] = 0x02;
//     builder.flag_bits_with_incompat(&incompat);
//     let err = parse_bytes(&builder.build()).unwrap_err();
//     assert!(matches!(err.error_type(), ParseErrorType::InvalidFlagBits),
//             "Unknown incompat flags should produce InvalidFlagBits");
// }

// #[test]
// fn error_variant_duplicate_format() {
//     let mut builder = ULogBuilder::new();
//     builder.flag_bits()
//            .format("dup", &[("uint64_t", "timestamp")])
//            .format("dup", &[("uint64_t", "timestamp")]);
//     builder.add_logged(0, 0, "dup");
//     builder.data(0, &0u64.to_le_bytes());
//     let err = parse_bytes(&builder.build()).unwrap_err();
//     assert!(matches!(err.error_type(), ParseErrorType::DuplicateFormat));
// }

// #[test]
// fn error_variant_duplicate_subscription() {
//     let mut builder = ULogBuilder::new();
//     builder.flag_bits()
//            .format("t", &[("uint64_t", "timestamp")])
//            .add_logged(0, 0, "t")
//            .add_logged(0, 1, "t");
//     builder.data(0, &0u64.to_le_bytes());
//     let err = parse_bytes(&builder.build()).unwrap_err();
//     assert!(matches!(err.error_type(), ParseErrorType::DuplicateSubscription));
// }

// #[test]
// fn error_variant_unknown_subscription() {
//     let mut builder = ULogBuilder::new();
//     builder.flag_bits()
//            .format("t", &[("uint64_t", "timestamp")])
//            .add_logged(0, 0, "t");
//     builder.data(99, &0u64.to_le_bytes());
//     let err = parse_bytes(&builder.build()).unwrap_err();
//     assert!(matches!(err.error_type(), ParseErrorType::UnknownSubscription));
// }

// #[test]
// fn error_variant_message_size_mismatch() {
//     let mut builder = ULogBuilder::new();
//     builder.flag_bits()
//            .format("t", &[("uint64_t", "timestamp"), ("float", "x")])
//            .add_logged(0, 0, "t");
//     builder.data(0, &[0u8; 4]); // too short
//     let err = parse_bytes(&builder.build()).unwrap_err();
//     assert!(matches!(err.error_type(), ParseErrorType::MessageSizeMismatch));
// }

// #[test]
// fn error_variant_invalid_format_string() {
//     let mut builder = ULogBuilder::new();
//     builder.flag_bits();
//     let mut bytes = builder.build();
//     let bad = b"no_colon_here";
//     bytes.extend_from_slice(&(bad.len() as u16).to_le_bytes());
//     bytes.push(b'F');
//     bytes.extend_from_slice(bad);
//     let err = parse_bytes(&bytes).unwrap_err();
//     assert!(matches!(err.error_type(), ParseErrorType::InvalidFormatString));
// }

// #[test]
// fn error_variant_truncated_message() {
//     // A message header that claims 100 bytes but the stream ends early.
//     // Currently the parser just stashes this as leftover and returns Ok.
//     // FUTURE: When the stream is finalized, this should be TruncatedMessage.
//     let mut builder = ULogBuilder::new();
//     builder.flag_bits();
//     let mut bytes = builder.build();
//     // Write a message header claiming 100 bytes, but only provide 5.
//     bytes.extend_from_slice(&100u16.to_le_bytes());
//     bytes.push(b'D');
//     bytes.extend_from_slice(&[0u8; 5]);
//     // TODO: parser.finalize() or similar should detect this truncation.
//     // For now, the parser treats it as incomplete and waits for more bytes.
//     // let err = parse_bytes_finalized(&bytes).unwrap_err();
//     // assert!(matches!(err.error_type(), ParseErrorType::TruncatedMessage));
// }

// =============================================================================
// 5. Error messages should be descriptive (contain useful context)
// =============================================================================
//
// These tests verify that the Debug output (the only inspection we have today)
// includes contextual information. Once Display is implemented, these should
// check format!("{}") instead.

#[test]
fn error_message_for_duplicate_subscription_contains_msg_id() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("accel", &[("uint64_t", "timestamp")])
        .add_logged(7, 0, "accel")
        .add_logged(7, 1, "accel");
    builder.data(7, &0u64.to_le_bytes());
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(result.is_err());
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("7"),
        "Error for duplicate msg_id should mention the msg_id (7), got: {}",
        debug
    );
}

#[test]
fn error_message_for_unregistered_msg_id_contains_id() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("gyro", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "gyro");
    builder.data(42, &0u64.to_le_bytes());
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(result.is_err());
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("42"),
        "Error for unregistered msg_id should mention 42, got: {}",
        debug
    );
}

#[test]
fn error_message_for_wrong_size_contains_size_info() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format(
            "baro",
            &[("uint64_t", "timestamp"), ("float", "pressure")],
        )
        .add_logged(0, 0, "baro");
    // Format expects 2 + 8 + 4 = 14 bytes. Send only 2 + 2 = 4 bytes.
    builder.data(0, &[0u8; 2]);
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(result.is_err());
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("size") || debug.contains("wrong"),
        "Error for size mismatch should mention 'size', got: {}",
        debug
    );
}

#[test]
fn error_message_for_duplicate_format_contains_name() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("mag", &[("uint64_t", "timestamp")])
        .format("mag", &[("uint64_t", "timestamp")]);
    builder.add_logged(0, 0, "mag");
    builder.data(0, &0u64.to_le_bytes());
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(result.is_err());
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("mag"),
        "Error for duplicate format should mention the format name 'mag', got: {}",
        debug
    );
}

#[test]
fn error_message_for_invalid_format_contains_offending_string() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    let invalid = b"bad_topic:notavalidtype";
    let msg_size = invalid.len() as u16;
    let mut bytes = builder.build();
    bytes.extend_from_slice(&msg_size.to_le_bytes());
    bytes.push(b'F');
    bytes.extend_from_slice(invalid);

    let result = parse_bytes(&bytes);
    assert!(result.is_err());
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("notavalidtype") || debug.contains("bad_topic") || debug.contains("invalid"),
        "Error for invalid format should contain the offending string, got: {}",
        debug
    );
}

// =============================================================================
// 6. Errors should propagate correctly (not be swallowed)
// =============================================================================

#[test]
fn consume_bytes_surfaces_invalid_header_error() {
    // LogParser::consume_bytes should return Err, not silently ignore.
    let bytes = vec![0xFF; 16];
    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "consume_bytes must not silently ignore invalid header"
    );
}

#[test]
fn consume_bytes_surfaces_incompatible_flags_error() {
    let mut builder = ULogBuilder::new();
    let mut incompat = [0u8; 8];
    incompat[1] = 0x01; // unknown flag in byte 1
    builder.flag_bits_with_incompat(&incompat);
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "consume_bytes must surface incompatible flag errors"
    );
}

#[test]
fn consume_bytes_surfaces_duplicate_format_error() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("dup_topic", &[("uint64_t", "timestamp")])
        .format("dup_topic", &[("uint64_t", "timestamp")]);
    builder.add_logged(0, 0, "dup_topic");
    builder.data(0, &0u64.to_le_bytes());
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "consume_bytes must surface duplicate format errors"
    );
}

#[test]
fn consume_bytes_surfaces_unregistered_msg_id_error() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("registered", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "registered");
    builder.data(55, &0u64.to_le_bytes());
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "consume_bytes must surface unregistered msg_id errors"
    );
}

// NOTE: read_file_with_simple_callback wraps UlogParseError into std::io::Error.
// This means the original error type is lost. Once UlogParseError implements
// std::error::Error, it should be possible to downcast:
//
// #[test]
// fn read_file_with_simple_callback_preserves_error_type() {
//     // Write a bad file to a temp path and parse it.
//     use std::io::Write;
//     let dir = std::env::temp_dir();
//     let path = dir.join("px4_ulog_test_bad_header.ulg");
//     let mut f = std::fs::File::create(&path).unwrap();
//     f.write_all(&[0xFF; 16]).unwrap();
//     drop(f);
//
//     let mut noop = |_: &px4_ulog::stream_parser::file_reader::Message| {
//         px4_ulog::stream_parser::file_reader::SimpleCallbackResult::KeepReading
//     };
//     let result = px4_ulog::stream_parser::file_reader::read_file_with_simple_callback(
//         path.to_str().unwrap(), &mut noop
//     );
//     assert!(result.is_err());
//     let io_err = result.unwrap_err();
//     // Currently the error is wrapped with format!("{:?}", e), losing type info.
//     // FUTURE: Should use .source() or downcast:
//     // let source = io_err.source().unwrap();
//     // assert!(source.downcast_ref::<UlogParseError>().is_some());
//     let msg = format!("{}", io_err);
//     assert!(msg.contains("InvalidFile") || msg.contains("header"),
//             "Wrapped io::Error should preserve error context, got: {}", msg);
//
//     std::fs::remove_file(&path).ok();
// }

// NOTE: full_parser::read_file has the same wrapping problem as
// read_file_with_simple_callback -- it converts UlogParseError to io::Error
// using format!("{:?}", e). Once UlogParseError implements std::error::Error,
// it should be passed as the source:
//
// #[test]
// fn full_parser_read_file_preserves_error_type() {
//     use std::io::Write;
//     let dir = std::env::temp_dir();
//     let path = dir.join("px4_ulog_test_full_parser_bad.ulg");
//     let mut f = std::fs::File::create(&path).unwrap();
//     f.write_all(&[0xFF; 16]).unwrap();
//     drop(f);
//
//     let result = px4_ulog::full_parser::read_file(path.to_str().unwrap());
//     assert!(result.is_err());
//     let io_err = result.unwrap_err();
//     // FUTURE: downcast to UlogParseError
//     // let source = io_err.source().unwrap();
//     // assert!(source.downcast_ref::<UlogParseError>().is_some());
//
//     std::fs::remove_file(&path).ok();
// }

// NOTE: The seek-based parser (dataset.rs) silently converts ALL errors to None
// in its Iterator impl (lines 82-86):
//
//     fn next(&mut self) -> Option<Self::Item> {
//         let data = get_next_data(self);
//         if let Ok(item) = data {
//             Some(item)
//         } else {
//             None  // <-- error is silently swallowed
//         }
//     }
//
// This means callers using the Iterator interface can never know if iteration
// stopped due to end-of-data vs a parse error. FUTURE: The iterator should
// yield Result<ULogData, Error> or provide a separate method to check for errors.
//
// #[test]
// fn seek_parser_iterator_surfaces_errors() {
//     use std::io::Write;
//     use px4_ulog::parser::dataset::ULogDatasetSource;
//     let dir = std::env::temp_dir();
//     let path = dir.join("px4_ulog_test_seek_corrupt.ulg");
//     // Write a file with valid header but corrupted data section.
//     let mut builder = ULogBuilder::new();
//     builder.flag_bits()
//            .format("test", &[("uint64_t", "timestamp")])
//            .add_logged(0, 0, "test");
//     // Write data with wrong size to trigger an error in the seek parser.
//     builder.data(0, &[0u8; 2]); // too short
//     let mut f = std::fs::File::create(&path).unwrap();
//     f.write_all(&builder.build()).unwrap();
//     drop(f);
//
//     let mut f = std::fs::File::open(&path).unwrap();
//     let dataset = f.get_dataset("test").unwrap();
//     // FUTURE: Iterator should yield Result items so errors are visible.
//     // Currently it just returns None, hiding the error.
//     let items: Vec<_> = dataset.collect();
//     // We expect 0 items (the data was corrupt), but we have no way to
//     // distinguish "no data" from "parse error". This is the bug.
//     assert_eq!(items.len(), 0);
//
//     std::fs::remove_file(&path).ok();
// }

// =============================================================================
// 6b. Verify errors propagate through file-based APIs using temp files
// =============================================================================

#[test]
fn read_file_with_simple_callback_returns_error_for_bad_file() {
    use px4_ulog::stream_parser::file_reader::{
        read_file_with_simple_callback, Message, SimpleCallbackResult,
    };
    use std::io::Write;

    let dir = std::env::temp_dir();
    let path = dir.join("px4_ulog_error_handling_test_bad.ulg");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&[0xFF; 16]).unwrap();
    }

    let mut noop = |_: &Message| SimpleCallbackResult::KeepReading;
    let result = read_file_with_simple_callback(path.to_str().unwrap(), &mut noop);
    assert!(
        result.is_err(),
        "read_file_with_simple_callback must return Err for invalid file, not silently succeed"
    );

    std::fs::remove_file(&path).ok();
}

#[test]
fn full_parser_read_file_returns_error_for_bad_file() {
    use std::io::Write;

    let dir = std::env::temp_dir();
    let path = dir.join("px4_ulog_error_handling_test_full_bad.ulg");
    {
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&[0xFF; 16]).unwrap();
    }

    let result = px4_ulog::full_parser::read_file(path.to_str().unwrap());
    assert!(
        result.is_err(),
        "full_parser::read_file must return Err for invalid file, not silently succeed"
    );

    std::fs::remove_file(&path).ok();
}

// =============================================================================
// Additional: edge cases for error conditions
// =============================================================================

#[test]
fn flag_bits_message_too_small_is_error() {
    // Flag bits message needs at least 40 bytes payload. Send less.
    let builder = ULogBuilder::new();
    let mut bytes = builder.build();
    // Write a flag bits message with only 10 bytes payload.
    bytes.extend_from_slice(&10u16.to_le_bytes());
    bytes.push(b'B');
    bytes.extend_from_slice(&[0u8; 10]);

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Undersized flag bits message must produce an error"
    );
}

#[test]
fn data_message_before_definitions_is_error() {
    // Sending a data message before any definitions/subscriptions.
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    // Skip format and add_logged, go straight to data.
    builder.data(0, &0u64.to_le_bytes());
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    // The parser transitions to data section on AddLoggedMessage or Data.
    // Data without a registered msg_id should fail.
    assert!(
        result.is_err(),
        "Data message without prior subscription must produce an error"
    );
}

#[test]
fn subscription_for_undefined_format_is_error() {
    // AddLoggedMessage referencing a format name that was never defined.
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("real_topic", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "nonexistent_topic");
    builder.data(0, &0u64.to_le_bytes());
    let bytes = builder.build();

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Subscription for undefined format must produce an error"
    );
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("nonexistent_topic") || debug.contains("format definition"),
        "Error should mention the missing format, got: {}",
        debug
    );
}

#[test]
fn duplicate_field_names_in_format_is_error() {
    // A format with two fields of the same name.
    let builder = ULogBuilder::new();
    // Manually build format string with duplicate field names.
    let format_str = b"dup_fields:uint64_t timestamp;float timestamp";
    let msg_size = format_str.len() as u16;
    let mut bytes = builder.build();
    bytes.extend_from_slice(&msg_size.to_le_bytes());
    bytes.push(b'F');
    bytes.extend_from_slice(format_str);

    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "Format with duplicate field names must produce an error"
    );
    let debug = format!("{:?}", result.unwrap_err());
    assert!(
        debug.contains("duplicate") || debug.contains("field"),
        "Error should mention duplicate field, got: {}",
        debug
    );
}
