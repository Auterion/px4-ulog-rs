//! Priority 1: Regression tests for known bugs in px4-ulog-rs.
//!
//! These tests pin current broken behavior so fixes can be verified.
//! Tests marked #[should_panic] or with comments about expected failures
//! document bugs that need fixing.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::LogParser;
use px4_ulog::stream_parser::model::{DataMessage, LoggedStringMessage, ParameterMessage};

/// Helper: parse bytes through LogParser and collect data messages.
fn parse_and_collect_data(bytes: &[u8]) -> Vec<(u16, Vec<u8>)> {
    let mut results: Vec<(u16, Vec<u8>)> = Vec::new();
    let mut cb = |msg: &DataMessage| {
        results.push((msg.msg_id, msg.data.to_vec()));
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut cb);
    parser.consume_bytes(bytes).expect("parse should not error");
    results
}

/// Helper: parse bytes and collect logged strings.
#[allow(dead_code)]
fn parse_and_collect_strings(bytes: &[u8]) -> Vec<(u8, u64, String)> {
    let mut results = Vec::new();
    let mut cb = |msg: &LoggedStringMessage| {
        results.push((msg.log_level, msg.timestamp, msg.logged_message.to_string()));
    };
    let mut parser = LogParser::default();
    parser.set_logged_string_message_callback(&mut cb);
    parser.consume_bytes(bytes).expect("parse should not error");
    results
}

/// Helper: parse bytes and collect parameters.
#[allow(dead_code)]
fn parse_and_collect_params(bytes: &[u8]) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let mut cb = |msg: &ParameterMessage| match msg {
        ParameterMessage::Int32(name, val, _) => {
            results.push((name.to_string(), format!("i32:{}", val)));
        }
        ParameterMessage::Float(name, val, _) => {
            results.push((name.to_string(), format!("f32:{}", val)));
        }
    };
    let mut parser = LogParser::default();
    parser.set_parameter_message_callback(&mut cb);
    parser.consume_bytes(bytes).expect("parse should not error");
    results
}

// =============================================================================
// P1-1: Off-by-one in parse_single_entry (file_reader.rs:203)
//
// When buf.len() == consumed_len (message fits exactly), the parser
// incorrectly treats it as needing more data. The condition should be
// `buf.len() < consumed_len`, not `buf.len() <= consumed_len`.
// =============================================================================

#[test]
fn test_off_by_one_exact_buffer_boundary() {
    let (builder, _msg_id) = ULogBuilder::minimal_with_data();
    let bytes = builder.build();

    // Feed the entire byte stream in one call — the last message should fit exactly.
    let results = parse_and_collect_data(&bytes);

    assert_eq!(results.len(), 1, "Expected 1 data message when it fits exactly in buffer");
}

#[test]
fn test_off_by_one_with_trailing_byte() {
    let (builder, _msg_id) = ULogBuilder::minimal_with_data();
    let mut bytes = builder.build();
    // Add one extra byte so buf.len() > consumed_len for the last message
    bytes.push(0x00);

    let results = parse_and_collect_data(&bytes);
    // With trailing byte, the off-by-one is not triggered
    assert_eq!(results.len(), 1, "Should parse 1 data message when there's a trailing byte");
}

// =============================================================================
// P1-2: Non-monotonic timestamps silently dropped (file_reader.rs:385-398)
//
// When a data message has timestamp <= last_timestamp, it is silently dropped.
// This loses valid data when timestamps are equal (same microsecond) or when
// data arrives slightly out of order.
// =============================================================================

#[test]
fn test_non_monotonic_timestamp_equal_dropped() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("test_topic", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "test_topic");

    // Three data messages: timestamps 100, 200, 200
    for (ts, x) in [(100u64, 1.0f32), (200u64, 2.0f32), (200u64, 3.0f32)] {
        let mut payload = Vec::new();
        payload.extend_from_slice(&ts.to_le_bytes());
        payload.extend_from_slice(&x.to_le_bytes());
        builder.data(0, &payload);
    }
    // Add trailing byte to avoid off-by-one
    let mut bytes = builder.build();
    bytes.push(0x00);

    let results = parse_and_collect_data(&bytes);

    assert_eq!(results.len(), 3, "All three data messages should be received");
}

#[test]
fn test_non_monotonic_timestamp_decreasing_dropped() {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("test_topic", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "test_topic");

    // Three data messages: timestamps 100, 200, 150 (decreasing)
    for (ts, x) in [(100u64, 1.0f32), (200u64, 2.0f32), (150u64, 3.0f32)] {
        let mut payload = Vec::new();
        payload.extend_from_slice(&ts.to_le_bytes());
        payload.extend_from_slice(&x.to_le_bytes());
        builder.data(0, &payload);
    }
    let mut bytes = builder.build();
    bytes.push(0x00);

    let results = parse_and_collect_data(&bytes);

    assert_eq!(results.len(), 3, "All three data messages should be received including out-of-order");
}

// =============================================================================
// P1-7: Unsafe f32 conversion (unpack/mod.rs:75)
//
// Uses raw pointer cast instead of f32::from_bits(). Verify correctness
// for special values before replacing the unsafe block.
// =============================================================================

#[test]
fn test_unsafe_f32_conversion_special_values() {
    use px4_ulog::unpack;

    // Zero
    assert_eq!(unpack::as_f32_le(&[0, 0, 0, 0]), 0.0f32);

    // Negative zero
    let neg_zero = unpack::as_f32_le(&[0, 0, 0, 0x80]);
    assert!(neg_zero.is_sign_negative() && neg_zero == 0.0);

    // One
    assert_eq!(unpack::as_f32_le(&1.0f32.to_le_bytes()), 1.0f32);

    // Negative one
    assert_eq!(unpack::as_f32_le(&(-1.0f32).to_le_bytes()), -1.0f32);

    // Infinity
    assert!(unpack::as_f32_le(&f32::INFINITY.to_le_bytes()).is_infinite());
    assert!(unpack::as_f32_le(&f32::INFINITY.to_le_bytes()).is_sign_positive());

    // Negative infinity
    assert!(unpack::as_f32_le(&f32::NEG_INFINITY.to_le_bytes()).is_infinite());
    assert!(unpack::as_f32_le(&f32::NEG_INFINITY.to_le_bytes()).is_sign_negative());

    // NaN — bit pattern should produce NaN
    let nan = unpack::as_f32_le(&f32::NAN.to_le_bytes());
    assert!(nan.is_nan());

    // Max / Min
    assert_eq!(unpack::as_f32_le(&f32::MAX.to_le_bytes()), f32::MAX);
    assert_eq!(unpack::as_f32_le(&f32::MIN.to_le_bytes()), f32::MIN);

    // Subnormal (smallest positive)
    let subnormal = f32::from_bits(1u32); // smallest subnormal
    assert_eq!(unpack::as_f32_le(&subnormal.to_le_bytes()), subnormal);

    // Cross-check: every value should match f32::from_bits
    for bits in [0u32, 1, 0x7F800000, 0xFF800000, 0x7FC00000, 0x80000000, 0x3F800000, 0xBF800000] {
        let from_unpack = unpack::as_f32_le(&bits.to_le_bytes());
        let from_std = f32::from_bits(bits);
        assert_eq!(from_unpack.to_bits(), from_std.to_bits(),
            "Mismatch for bits 0x{:08X}: unpack={}, std={}", bits, from_unpack, from_std);
    }
}
