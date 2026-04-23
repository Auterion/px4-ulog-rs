//! Tests verifying that Info ('I') messages are received both in the definitions
//! section and in the data section of a ULog file.
//!
//! Per the ULog spec, Info messages can appear in both sections. Some ULog writers
//! emit Info messages after the data section starts (e.g., late-arriving metadata).
//! These tests ensure the parser does not silently drop or error on such messages.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, Message, SimpleCallbackResult,
};

/// Helper: write bytes to a temp file, parse with read_file_with_simple_callback,
/// and collect all InfoMessage (key, value) pairs.
fn collect_info_messages(bytes: &[u8], test_name: &str) -> Vec<(String, Vec<u8>)> {
    let tmp = std::env::temp_dir().join(format!("{}.ulg", test_name));
    std::fs::write(&tmp, bytes).expect("write temp file");

    let mut info_messages: Vec<(String, Vec<u8>)> = Vec::new();
    let result = read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg: &Message| {
        if let Message::InfoMessage(info) = msg {
            info_messages.push((info.key.to_string(), info.value.to_vec()));
        }
        SimpleCallbackResult::KeepReading
    });
    assert!(result.is_ok(), "parse failed: {:?}", result.err());

    // Clean up
    let _ = std::fs::remove_file(&tmp);

    info_messages
}

// =============================================================================
// 1. Baseline: Info in definitions section works
// =============================================================================

#[test]
fn test_info_before_data_messages() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    // Info in definitions section (before any data-triggering message)
    builder.info("char", "sys_name", b"PX4");
    builder.info("char", "ver_hw", b"v5");
    // Add format + subscription + data to make a complete stream
    builder
        .format("sensor", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "sensor");
    let mut data_payload = Vec::new();
    data_payload.extend_from_slice(&1000u64.to_le_bytes());
    data_payload.extend_from_slice(&1.5f32.to_le_bytes());
    builder.data(0, &data_payload);

    let infos = collect_info_messages(&builder.build(), "test_info_before_data");

    assert_eq!(infos.len(), 2, "expected 2 info messages, got {:?}", infos);
    assert_eq!(infos[0].0, "sys_name");
    assert_eq!(infos[0].1, b"PX4");
    assert_eq!(infos[1].0, "ver_hw");
    assert_eq!(infos[1].1, b"v5");
}

// =============================================================================
// 2. Info message after Data messages should still be received
// =============================================================================

#[test]
fn test_info_after_data_messages() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder
        .format("sensor", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "sensor");

    // Emit a data message (this transitions parser to InData state)
    let mut data_payload = Vec::new();
    data_payload.extend_from_slice(&1000u64.to_le_bytes());
    data_payload.extend_from_slice(&1.5f32.to_le_bytes());
    builder.data(0, &data_payload);

    // Now emit Info in the data section
    builder.info("char", "late_metadata", b"arrived_late");

    let infos = collect_info_messages(&builder.build(), "test_info_after_data");

    assert_eq!(
        infos.len(),
        1,
        "expected 1 info message in data section, got {:?}",
        infos
    );
    assert_eq!(infos[0].0, "late_metadata");
    assert_eq!(infos[0].1, b"arrived_late");
}

// =============================================================================
// 3. Info messages interleaved between Data messages should all be received
// =============================================================================

#[test]
fn test_info_interleaved_with_data() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder
        .format("sensor", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "sensor");

    // Data, then Info, then Data, then Info
    let mut data1 = Vec::new();
    data1.extend_from_slice(&1000u64.to_le_bytes());
    data1.extend_from_slice(&1.0f32.to_le_bytes());
    builder.data(0, &data1);

    builder.info("char", "info_one", b"first");

    let mut data2 = Vec::new();
    data2.extend_from_slice(&2000u64.to_le_bytes());
    data2.extend_from_slice(&2.0f32.to_le_bytes());
    builder.data(0, &data2);

    builder.info("char", "info_two", b"second");

    let mut data3 = Vec::new();
    data3.extend_from_slice(&3000u64.to_le_bytes());
    data3.extend_from_slice(&3.0f32.to_le_bytes());
    builder.data(0, &data3);

    builder.info("char", "info_three", b"third");

    let infos = collect_info_messages(&builder.build(), "test_info_interleaved");

    assert_eq!(
        infos.len(),
        3,
        "expected 3 interleaved info messages, got {:?}",
        infos
    );
    assert_eq!(infos[0].0, "info_one");
    assert_eq!(infos[0].1, b"first");
    assert_eq!(infos[1].0, "info_two");
    assert_eq!(infos[1].1, b"second");
    assert_eq!(infos[2].0, "info_three");
    assert_eq!(infos[2].1, b"third");
}

// =============================================================================
// 4. Info in definitions AND data section — verify all received with correct values
// =============================================================================

#[test]
fn test_info_in_both_sections() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();

    // Info in definitions section
    builder.info("char", "def_key", b"def_value");
    builder.info("int32_t", "def_number", &42i32.to_le_bytes());

    // Transition to data section
    builder
        .format("sensor", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "sensor");

    let mut data_payload = Vec::new();
    data_payload.extend_from_slice(&1000u64.to_le_bytes());
    data_payload.extend_from_slice(&1.5f32.to_le_bytes());
    builder.data(0, &data_payload);

    // Info in data section
    builder.info("char", "data_key", b"data_value");
    builder.info("float", "data_float", &std::f32::consts::PI.to_le_bytes());

    let infos = collect_info_messages(&builder.build(), "test_info_both_sections");

    assert_eq!(
        infos.len(),
        4,
        "expected 4 info messages total, got {:?}",
        infos
    );

    // Definitions section info
    assert_eq!(infos[0].0, "def_key");
    assert_eq!(infos[0].1, b"def_value");
    assert_eq!(infos[1].0, "def_number");
    assert_eq!(infos[1].1, &42i32.to_le_bytes());

    // Data section info
    assert_eq!(infos[2].0, "data_key");
    assert_eq!(infos[2].1, b"data_value");
    assert_eq!(infos[3].0, "data_float");
    assert_eq!(infos[3].1, &std::f32::consts::PI.to_le_bytes());
}
