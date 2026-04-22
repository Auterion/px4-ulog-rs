//! Priority 4: Edge case and corruption tests.
//!
//! These verify the parser does not panic or hang on malformed input.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::LogParser;
use px4_ulog::stream_parser::model::DataMessage;

fn parse_bytes(bytes: &[u8]) -> Result<(), String> {
    let mut noop = |_: &DataMessage| {};
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut noop);
    parser
        .consume_bytes(bytes)
        .map_err(|e| format!("{:?}", e))
}

// =============================================================================
// Empty and minimal files
// =============================================================================

#[test]
fn test_empty_file() {
    let result = parse_bytes(&[]);
    assert!(result.is_ok(), "Empty input should not error");
}

#[test]
fn test_only_header_no_messages() {
    let builder = ULogBuilder::new();
    let bytes = builder.build();
    assert_eq!(bytes.len(), 16);
    let result = parse_bytes(&bytes);
    assert!(result.is_ok(), "Header-only input should not error");
}

#[test]
fn test_header_plus_one_byte() {
    let mut bytes = ULogBuilder::new().build();
    bytes.push(0x00); // partial message header
    let result = parse_bytes(&bytes);
    assert!(result.is_ok(), "Partial message header should stash as leftover, not error");
}

#[test]
fn test_header_plus_two_bytes() {
    let mut bytes = ULogBuilder::new().build();
    bytes.extend_from_slice(&[0x00, 0x00]); // 2 of 3 message header bytes
    let result = parse_bytes(&bytes);
    assert!(result.is_ok(), "Incomplete message header should not error");
}

// =============================================================================
// Truncated messages
// =============================================================================

#[test]
fn test_truncated_message_body() {
    let mut bytes = ULogBuilder::new().build();
    // Message header claiming 100 bytes, but only 50 bytes of body
    bytes.extend_from_slice(&100u16.to_le_bytes());
    bytes.push(b'B'); // FlagBits type
    bytes.extend_from_slice(&[0u8; 50]); // only 50 of 100 bytes
    let result = parse_bytes(&bytes);
    // Should stash in leftover, not error (waiting for more data)
    assert!(result.is_ok(), "Truncated message body should be stashed as leftover");
}

#[test]
fn test_zero_length_message() {
    let mut bytes = ULogBuilder::new().build();
    // Message with msg_size = 0
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.push(b'B');
    // Add trailing bytes so parser can proceed
    bytes.extend_from_slice(&[0u8; 10]);
    let result = parse_bytes(&bytes);
    // Should not panic or infinite loop
    // May error (flag bits too small) — that's fine
    let _ = result;
}

#[test]
fn test_max_length_message_header() {
    let mut bytes = ULogBuilder::new().build();
    // Message claiming max size (65535 bytes) but no body
    bytes.extend_from_slice(&0xFFFFu16.to_le_bytes());
    bytes.push(b'F');
    // Don't provide the body — should stash as leftover
    let result = parse_bytes(&bytes);
    assert!(result.is_ok(), "Max-length message header without body should be stashed");
}

// =============================================================================
// Invalid headers
// =============================================================================

#[test]
fn test_wrong_magic_bytes() {
    let bytes = vec![0x00; 16]; // all zeros, wrong magic
    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "Wrong magic should error");
}

#[test]
fn test_all_zeros_file() {
    let bytes = vec![0u8; 1024];
    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "All-zero file should error on header");
}

#[test]
fn test_random_garbage_file() {
    // Fixed seed for reproducibility
    let mut bytes = Vec::with_capacity(1024);
    let mut state: u32 = 0xDEADBEEF;
    for _ in 0..1024 {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        bytes.push((state >> 16) as u8);
    }
    let result = parse_bytes(&bytes);
    // Should error on invalid header, not panic
    assert!(result.is_err(), "Random garbage should error on header");
}

#[test]
fn test_almost_valid_magic() {
    let mut bytes = ULogBuilder::new().build();
    bytes[6] = 0x00; // corrupt last magic byte
    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "Corrupted magic byte should error");
}

// =============================================================================
// Invalid message sequences
// =============================================================================

#[test]
fn test_data_message_before_flag_bits() {
    let mut builder = ULogBuilder::new();
    // Skip flag_bits, go straight to data
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u16.to_le_bytes()); // msg_id
    payload.extend_from_slice(&[0u8; 12]); // some data
    builder.data_raw(&payload);
    let mut bytes = builder.build();
    bytes.push(0x00); // trailing byte
    let result = parse_bytes(&bytes);
    // Should error — data before definitions
    assert!(result.is_err(), "Data before flag_bits should error");
}

#[test]
fn test_format_before_flag_bits() {
    let mut builder = ULogBuilder::new();
    // Format message before flag_bits
    builder.format("test", &[("uint64_t", "timestamp")]);
    let mut bytes = builder.build();
    bytes.push(0x00);
    // The parser currently allows this (Format transitions to InDefinitions from AfterHeader
    // via the Parameter path). This test documents current behavior.
    let _result = parse_bytes(&bytes);
}

#[test]
fn test_flag_bits_not_first_message() {
    let mut builder = ULogBuilder::new();
    // Parameter before flag_bits (should be ok per current impl)
    builder.parameter_i32("TEST_PARAM", 42);
    builder.flag_bits();
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    // Current impl: Parameter transitions AfterHeader -> InDefinitions,
    // then FlagBits in InDefinitions should error.
    // Document whatever happens:
    if result.is_err() {
        // Expected: flag_bits at bad position
    }
}

#[test]
fn test_data_message_unregistered_msg_id() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("test_topic", &[("uint64_t", "timestamp"), ("float", "x")]);
    // Add subscription for msg_id=0
    builder.add_logged(0, 0, "test_topic");
    // Data message for msg_id=99 (never registered)
    let mut payload = Vec::new();
    payload.extend_from_slice(&99u16.to_le_bytes());
    payload.extend_from_slice(&1000u64.to_le_bytes());
    payload.extend_from_slice(&1.0f32.to_le_bytes());
    builder.data_raw(&payload);
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "Unregistered msg_id should error");
}

#[test]
fn test_data_message_wrong_size() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("test_topic", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "test_topic");
    // Data message with wrong payload size (too short)
    let mut payload = Vec::new();
    payload.extend_from_slice(&0u16.to_le_bytes()); // msg_id
    payload.extend_from_slice(&1000u64.to_le_bytes()); // timestamp only, missing float
    builder.data_raw(&payload);
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "Data message with wrong size should error");
}

// =============================================================================
// Invalid format strings
// =============================================================================

#[test]
fn test_corrupt_format_no_colon() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    // Raw format message without colon separator
    builder.unknown_message(b'F', b"invalid_format_string");
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "Format without colon should error");
}

#[test]
fn test_corrupt_format_empty_name() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.unknown_message(b'F', b":uint64_t timestamp");
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "Format with empty name should error");
}

#[test]
fn test_format_duplicate_field_name() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.unknown_message(b'F', b"dup:uint64_t x;float x");
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "Format with duplicate field should error");
}

#[test]
fn test_format_empty_fields() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.unknown_message(b'F', b"empty:");
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    // Empty fields list — should not panic
    let _ = result;
}

// =============================================================================
// Flag bits edge cases
// =============================================================================

#[test]
fn test_flag_bits_incompatible_flags_reject() {
    let mut builder = ULogBuilder::new();
    // Set unknown incompat flag in byte 1
    let mut incompat = [0u8; 8];
    incompat[1] = 0xFF;
    builder.flag_bits_with_incompat(&incompat);
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "Unknown incompat flags should error");
}

#[test]
fn test_flag_bits_data_appended_flag_accepted() {
    let mut builder = ULogBuilder::new();
    let mut incompat = [0u8; 8];
    incompat[0] = 0x01; // DATA_APPENDED bit only
    builder.flag_bits_with_incompat(&incompat);
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_ok(), "DATA_APPENDED flag alone should be accepted");
}

#[test]
fn test_flag_bits_too_short() {
    let mut builder = ULogBuilder::new();
    // FlagBits with only 39 bytes (needs 40)
    builder.unknown_message(b'B', &[0u8; 39]);
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "FlagBits shorter than 40 bytes should error");
}

// =============================================================================
// Unknown message types
// =============================================================================

#[test]
fn test_unknown_message_type_ignored() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.unknown_message(0xFF, &[0u8; 10]);
    builder.format("test", &[("uint64_t", "timestamp")]);
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_ok(), "Unknown message type should be silently skipped");
}

// =============================================================================
// Currently-ignored message types (document as known gaps)
// =============================================================================

#[test]
fn test_info_message_silently_ignored() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.info("char", "sys_name", b"PX4");
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_ok(), "Info message should be silently ignored (known gap)");
}

#[test]
fn test_dropout_message_silently_ignored() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("test", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "test");
    builder.dropout(100);
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_ok(), "Dropout message should be silently ignored (known gap)");
}

#[test]
fn test_sync_message_silently_ignored() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("test", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "test");
    builder.sync();
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_ok(), "Sync message should be silently ignored (known gap)");
}

#[test]
fn test_remove_logged_silently_ignored() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("test", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "test");
    builder.remove_logged(0);
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_ok(), "RemoveLogged should be silently ignored (known gap)");
}

#[test]
fn test_tagged_logged_string_silently_ignored() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("test", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "test");
    builder.tagged_logged_string(0x33, 1, 1000, "test message");
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(result.is_ok(), "TaggedLoggedString should be silently ignored (known gap)");
}

// =============================================================================
// Not a log file
// =============================================================================

#[test]
fn test_not_a_log_file_fixture() {
    let path = format!(
        "{}/tests/fixtures/not_a_log_file.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let bytes = std::fs::read(&path).unwrap();
    // Empty file (0 bytes) — should not error, just nothing to parse
    if bytes.is_empty() {
        let result = parse_bytes(&bytes);
        assert!(result.is_ok(), "Empty file should not error");
    } else {
        let result = parse_bytes(&bytes);
        assert!(result.is_err(), "Non-ULog file should error on header");
    }
}
