//! Tests for timestamp field type handling across all ULog-spec-valid types.
//!
//! The ULog specification allows timestamp fields to be:
//! - uint64_t: microseconds (standard, most common)
//! - uint32_t: microseconds (truncated)
//! - uint16_t: microseconds (truncated)
//! - uint8_t:  milliseconds (per spec — different unit!)
//!
//! IMPORTANT: The parser does NOT normalize timestamp units. A uint8 timestamp
//! field stores milliseconds, but `parse_timestamp()` returns the raw value
//! cast to u64 — the same return type as uint64 timestamps which are in
//! microseconds. Consumers must check `TimestampFieldType` to interpret the
//! value correctly.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::LogParser;
use px4_ulog::stream_parser::model::{DataMessage, TimestampFieldType};

/// Helper: build a ULog stream with a given timestamp type, parse it, and
/// return the (TimestampFieldType, parsed_timestamp_u64, raw_data_bytes) from
/// the first data message.
fn parse_with_timestamp_type(
    ts_type_str: &str,
    ts_bytes: &[u8],
    extra_field_bytes: &[u8],
) -> (TimestampFieldType, u64) {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format(
            "sensor_data",
            &[(ts_type_str, "timestamp"), ("float", "value")],
        )
        .add_logged(0, 0, "sensor_data");

    // Build field data: timestamp bytes + extra field bytes
    let mut field_data = Vec::new();
    field_data.extend_from_slice(ts_bytes);
    field_data.extend_from_slice(extra_field_bytes);
    builder.data(0, &field_data);

    let bytes = builder.build();

    let mut result_ts_type: Option<TimestampFieldType> = None;
    let mut result_ts_value: Option<u64> = None;

    let mut cb = |msg: &DataMessage| {
        let fmt = msg.flattened_format;
        let ts_field = fmt.timestamp_field.as_ref().expect("should have timestamp field");
        result_ts_type = Some(ts_field.field_type.clone());
        result_ts_value = Some(ts_field.parse_timestamp(msg.data));
    };

    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut cb);
    parser.consume_bytes(&bytes).expect("parse should succeed");

    (
        result_ts_type.expect("data callback was not called"),
        result_ts_value.expect("data callback was not called"),
    )
}

#[test]
fn test_uint64_timestamp() {
    // Standard case: uint64_t timestamp in microseconds
    let ts_val: u64 = 123_456_789;
    let extra = 1.5f32.to_le_bytes();

    let (field_type, parsed) = parse_with_timestamp_type(
        "uint64_t",
        &ts_val.to_le_bytes(),
        &extra,
    );

    assert_eq!(field_type, TimestampFieldType::UInt64);
    assert_eq!(parsed, ts_val);
}

#[test]
fn test_uint32_timestamp() {
    let ts_val: u32 = 1_000_000; // 1 second in microseconds
    let extra = 2.5f32.to_le_bytes();

    let (field_type, parsed) = parse_with_timestamp_type(
        "uint32_t",
        &ts_val.to_le_bytes(),
        &extra,
    );

    assert_eq!(field_type, TimestampFieldType::UInt32);
    assert_eq!(parsed, ts_val as u64);
}

#[test]
fn test_uint16_timestamp() {
    let ts_val: u16 = 50_000; // 50ms in microseconds
    let extra = 3.5f32.to_le_bytes();

    let (field_type, parsed) = parse_with_timestamp_type(
        "uint16_t",
        &ts_val.to_le_bytes(),
        &extra,
    );

    assert_eq!(field_type, TimestampFieldType::UInt16);
    assert_eq!(parsed, ts_val as u64);
}

#[test]
fn test_uint8_timestamp() {
    // uint8_t timestamps are in MILLISECONDS per the ULog spec.
    // The parser returns the raw value (e.g., 200 meaning 200ms)
    // without converting to microseconds.
    let ts_val: u8 = 200; // 200 milliseconds
    let extra = 4.5f32.to_le_bytes();

    let (field_type, parsed) = parse_with_timestamp_type(
        "uint8_t",
        &[ts_val],
        &extra,
    );

    assert_eq!(field_type, TimestampFieldType::UInt8);
    assert_eq!(parsed, ts_val as u64);
}

#[test]
fn test_timestamp_values_preserved() {
    // Verify that specific timestamp values round-trip correctly for each type.
    // The parser should preserve the exact encoded value with no transformation.

    // uint64: large microsecond timestamp (roughly 16 minutes)
    let ts64: u64 = 1_000_000_000;
    let extra = 0.0f32.to_le_bytes();
    let (_, parsed) = parse_with_timestamp_type("uint64_t", &ts64.to_le_bytes(), &extra);
    assert_eq!(parsed, 1_000_000_000);

    // uint32: max value (~71 minutes in microseconds)
    let ts32: u32 = u32::MAX;
    let (_, parsed) = parse_with_timestamp_type("uint32_t", &ts32.to_le_bytes(), &extra);
    assert_eq!(parsed, u32::MAX as u64);

    // uint16: max value (~65ms in microseconds)
    let ts16: u16 = u16::MAX;
    let (_, parsed) = parse_with_timestamp_type("uint16_t", &ts16.to_le_bytes(), &extra);
    assert_eq!(parsed, u16::MAX as u64);

    // uint8: max value (255 milliseconds — note: different unit!)
    let ts8: u8 = u8::MAX;
    let (_, parsed) = parse_with_timestamp_type("uint8_t", &[ts8], &extra);
    assert_eq!(parsed, u8::MAX as u64);
}

#[test]
fn test_uint8_timestamp_unit_documentation() {
    // This test documents the current behavior and the unit mismatch.
    //
    // Per the ULog spec:
    // - uint8_t timestamps are in MILLISECONDS
    // - uint16_t/uint32_t/uint64_t timestamps are in MICROSECONDS
    //
    // The parser's `parse_timestamp()` always returns a raw u64 with NO
    // unit normalization. This means:
    //
    //   - For uint64/uint32/uint16 timestamps: returned value is in microseconds
    //   - For uint8 timestamps: returned value is in MILLISECONDS
    //
    // Consumers that assume all timestamps are microseconds will misinterpret
    // uint8 timestamps by a factor of 1000x. For example, a uint8 timestamp
    // of 100 means 100ms (= 100,000 microseconds), but parse_timestamp()
    // returns 100 — which would be interpreted as 100 microseconds (= 0.1ms).
    //
    // To correctly handle uint8 timestamps, consumers must either:
    // 1. Check `timestamp_field.field_type` and multiply by 1000 if UInt8
    // 2. Or the parser should be updated to normalize all timestamps to
    //    microseconds before returning.

    let ts_val: u8 = 100; // 100 milliseconds per spec
    let extra = 0.0f32.to_le_bytes();

    let (field_type, parsed) = parse_with_timestamp_type("uint8_t", &[ts_val], &extra);

    // The parser returns the raw value — no normalization
    assert_eq!(parsed, 100);
    assert_eq!(field_type, TimestampFieldType::UInt8);

    // If this were normalized to microseconds, it would be 100_000.
    // This assertion documents that normalization does NOT happen:
    assert_ne!(parsed, 100_000, "Parser should NOT normalize uint8 timestamps (currently). \
        If this assertion fails, it means normalization was added — update this test.");

    // Demonstrate the correct interpretation for consumers:
    let microseconds = match field_type {
        TimestampFieldType::UInt8 => parsed * 1000, // milliseconds -> microseconds
        _ => parsed,                                 // already microseconds
    };
    assert_eq!(microseconds, 100_000);
}
