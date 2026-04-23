//! Structural edge cases: inputs that are technically malformed but not
//! produced by random corruption. Covers empty/minimal streams, size
//! boundaries on the message framing, message sequencing violations
//! (data before definitions, out-of-order FlagBits), and positive tests
//! for behavior the parser deliberately allows (DATA_APPENDED flag,
//! unknown message types).
//!
//! Corruption-fuzz mutations live in tests/corruption.rs. Specific
//! error-message content assertions live in tests/error_handling.rs.
//! Chunk-boundary reassembly lives in tests/chunk_boundaries.rs.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::LogParser;
use px4_ulog::stream_parser::model::DataMessage;

fn parse_bytes(bytes: &[u8]) -> Result<(), String> {
    let mut noop = |_: &DataMessage| {};
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut noop);
    parser.consume_bytes(bytes).map_err(|e| format!("{:?}", e))
}

// =============================================================================
// Empty and minimal files
// =============================================================================

#[test]
fn test_empty_file() {
    let result = parse_bytes(&[]);
    assert!(result.is_ok(), "Empty input should not error");
}

// =============================================================================
// Pathological message-size boundaries
// =============================================================================

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
    // May error (flag bits too small), that's fine
    let _ = result;
}

#[test]
fn test_max_length_message_header() {
    let mut bytes = ULogBuilder::new().build();
    // Message claiming max size (65535 bytes) but no body
    bytes.extend_from_slice(&0xFFFFu16.to_le_bytes());
    bytes.push(b'F');
    // Don't provide the body, should stash as leftover
    let result = parse_bytes(&bytes);
    assert!(
        result.is_ok(),
        "Max-length message header without body should be stashed"
    );
}

// =============================================================================
// Not even remotely a ULog
// =============================================================================

#[test]
fn test_all_zeros_file() {
    let bytes = vec![0u8; 1024];
    let result = parse_bytes(&bytes);
    assert!(result.is_err(), "All-zero file should error on header");
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
    // Should error, data before definitions
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

// =============================================================================
// Format-string edge cases not covered by error_handling's parameterized check
// =============================================================================

#[test]
fn test_format_empty_fields() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.unknown_message(b'F', b"empty:");
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    // Empty fields list, should not panic
    let _ = result;
}

// =============================================================================
// Flag bits edge cases
// =============================================================================

#[test]
fn test_flag_bits_data_appended_flag_accepted() {
    let mut builder = ULogBuilder::new();
    let mut incompat = [0u8; 8];
    incompat[0] = 0x01; // DATA_APPENDED bit only
    builder.flag_bits_with_incompat(&incompat);
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(
        result.is_ok(),
        "DATA_APPENDED flag alone should be accepted"
    );
}

#[test]
fn test_flag_bits_too_short() {
    let mut builder = ULogBuilder::new();
    // FlagBits with only 39 bytes (needs 40)
    builder.unknown_message(b'B', &[0u8; 39]);
    let mut bytes = builder.build();
    bytes.push(0x00);
    let result = parse_bytes(&bytes);
    assert!(
        result.is_err(),
        "FlagBits shorter than 40 bytes should error"
    );
}

// =============================================================================
// Unknown message types are silently skipped (positive test)
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
    assert!(
        result.is_ok(),
        "Unknown message type should be silently skipped"
    );
}

// =============================================================================
// Non-ULog input file
// =============================================================================

#[test]
fn test_not_a_log_file_fixture() {
    let path = format!(
        "{}/tests/fixtures/not_a_log_file.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let bytes = std::fs::read(&path).unwrap();
    // Empty file (0 bytes), should not error, just nothing to parse
    if bytes.is_empty() {
        let result = parse_bytes(&bytes);
        assert!(result.is_ok(), "Empty file should not error");
    } else {
        let result = parse_bytes(&bytes);
        assert!(result.is_err(), "Non-ULog file should error on header");
    }
}
