//! Tests for parameter change handling during flight.
//!
//! ULog Parameter ('P') messages can appear in both the definitions section (initial values)
//! and the data section (values changed mid-flight). The parser tracks which stage each
//! parameter belongs to via LogStage::Definitions vs LogStage::Data.
//!
//! pyulog separates these as `initial_parameters` vs `changed_parameters` (with timestamps).
//! These tests verify that px4-ulog-rs correctly distinguishes between the two.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, Message, SimpleCallbackResult,
};
use px4_ulog::stream_parser::model::{LogStage, ParameterMessage};

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

/// Helper to extract (name, value_as_string, log_stage) from a ParameterMessage.
fn extract_param_info(pm: &ParameterMessage) -> (String, String, LogStage) {
    match pm {
        ParameterMessage::Float(name, val, stage) => {
            (name.to_string(), format!("{}", val), stage.clone())
        }
        ParameterMessage::Int32(name, val, stage) => {
            (name.to_string(), format!("{}", val), stage.clone())
        }
    }
}

// =============================================================================
// Test 1: Parameters in the definitions section get LogStage::Definitions
// =============================================================================

#[test]
fn test_initial_parameters_in_definitions() {
    // Build a ULog with parameters BEFORE any data messages (i.e., in definitions section).
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.parameter_i32("SYS_AUTOSTART", 4001);
    builder.parameter_f32("MC_ROLLRATE_P", 0.15);

    // Add a format + subscription + data message to trigger transition to data section.
    builder
        .format("test_topic", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "test_topic")
        .data(0, &1000u64.to_le_bytes());

    let tmp = std::env::temp_dir().join("test_initial_params_in_defs.ulg");
    std::fs::write(&tmp, builder.build()).expect("write temp file");

    let mut params: Vec<(String, String, LogStage)> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        if let Message::ParameterMessage(pm) = msg {
            params.push(extract_param_info(pm));
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(params.len(), 2, "Expected 2 initial parameters");
    assert_eq!(params[0].0, "SYS_AUTOSTART");
    assert_eq!(params[0].2, LogStage::Definitions);
    assert_eq!(params[1].0, "MC_ROLLRATE_P");
    assert_eq!(params[1].2, LogStage::Definitions);
}

// =============================================================================
// Test 2: Parameters after data section starts get LogStage::Data
// =============================================================================

#[test]
fn test_changed_parameters_in_data_section() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();

    // Initial parameter in definitions.
    builder.parameter_i32("SYS_AUTOSTART", 4001);

    // Format + subscription to define data messages.
    builder
        .format("test_topic", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "test_topic");

    // Data message triggers transition to data section.
    builder.data(0, &1000u64.to_le_bytes());

    // Parameter change AFTER data section has started.
    builder.parameter_i32("SYS_AUTOSTART", 4002);
    builder.parameter_f32("MC_ROLLRATE_P", 0.20);

    let tmp = std::env::temp_dir().join("test_changed_params_data_section.ulg");
    std::fs::write(&tmp, builder.build()).expect("write temp file");

    let mut params: Vec<(String, String, LogStage)> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        if let Message::ParameterMessage(pm) = msg {
            params.push(extract_param_info(pm));
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(params.len(), 3, "Expected 1 initial + 2 changed parameters");

    // First param: definitions section.
    assert_eq!(params[0].0, "SYS_AUTOSTART");
    assert_eq!(params[0].1, "4001");
    assert_eq!(params[0].2, LogStage::Definitions);

    // Second param: data section (changed).
    assert_eq!(params[1].0, "SYS_AUTOSTART");
    assert_eq!(params[1].1, "4002");
    assert_eq!(params[1].2, LogStage::Data);

    // Third param: data section (changed).
    assert_eq!(params[2].0, "MC_ROLLRATE_P");
    assert_eq!(params[2].2, LogStage::Data);
}

// =============================================================================
// Test 3: Same parameter changed twice (definitions then data section)
// =============================================================================

#[test]
fn test_same_parameter_changed_twice() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();

    // Initial value in definitions.
    builder.parameter_f32("MPC_Z_VEL_MAX_DN", 1.5);

    // Set up data section.
    builder
        .format("test_topic", &[("uint64_t", "timestamp")])
        .add_logged(0, 0, "test_topic")
        .data(0, &1000u64.to_le_bytes());

    // First in-flight change.
    builder.parameter_f32("MPC_Z_VEL_MAX_DN", 1.0);

    // More data.
    builder.data(0, &2000u64.to_le_bytes());

    // Second in-flight change.
    builder.parameter_f32("MPC_Z_VEL_MAX_DN", 2.0);

    let tmp = std::env::temp_dir().join("test_same_param_changed_twice.ulg");
    std::fs::write(&tmp, builder.build()).expect("write temp file");

    let mut params: Vec<(String, String, LogStage)> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        if let Message::ParameterMessage(pm) = msg {
            params.push(extract_param_info(pm));
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(params.len(), 3, "Expected 1 initial + 2 in-flight changes");

    assert_eq!(params[0].0, "MPC_Z_VEL_MAX_DN");
    assert_eq!(params[0].1, "1.5");
    assert_eq!(params[0].2, LogStage::Definitions);

    assert_eq!(params[1].0, "MPC_Z_VEL_MAX_DN");
    assert_eq!(params[1].1, "1");
    assert_eq!(params[1].2, LogStage::Data);

    assert_eq!(params[2].0, "MPC_Z_VEL_MAX_DN");
    assert_eq!(params[2].1, "2");
    assert_eq!(params[2].2, LogStage::Data);
}

// =============================================================================
// Test 4: Parameter changes are interleaved with data messages in callback order
// =============================================================================

#[test]
fn test_parameter_change_with_timestamp_context() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();

    // Initial parameter.
    builder.parameter_i32("NAV_RCL_ACT", 2);

    // Set up data section.
    builder
        .format("test_topic", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "test_topic");

    // Data at t=1000.
    let mut data1 = Vec::new();
    data1.extend_from_slice(&1000u64.to_le_bytes());
    data1.extend_from_slice(&1.0f32.to_le_bytes());
    builder.data(0, &data1);

    // Parameter change between data messages.
    builder.parameter_i32("NAV_RCL_ACT", 3);

    // Data at t=2000.
    let mut data2 = Vec::new();
    data2.extend_from_slice(&2000u64.to_le_bytes());
    data2.extend_from_slice(&2.0f32.to_le_bytes());
    builder.data(0, &data2);

    let tmp = std::env::temp_dir().join("test_param_change_with_ts.ulg");
    std::fs::write(&tmp, builder.build()).expect("write temp file");

    // Collect message order as a sequence of tags.
    let mut events: Vec<String> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::ParameterMessage(pm) => {
                let (name, val, stage) = extract_param_info(pm);
                events.push(format!("Param({},{},{:?})", name, val, stage));
            }
            Message::Data(_) => {
                events.push("Data".to_string());
            }
            _ => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    // Expected order: initial param -> data -> changed param -> data.
    assert_eq!(events.len(), 4, "Expected 4 events: {:?}", events);
    assert_eq!(events[0], "Param(NAV_RCL_ACT,2,Definitions)");
    assert_eq!(events[1], "Data");
    assert_eq!(events[2], "Param(NAV_RCL_ACT,3,Data)");
    assert_eq!(events[3], "Data");
}

// =============================================================================
// Test 5: sample.ulg has 6 changed parameters (matching pyulog)
// =============================================================================

/// pyulog reports for sample.ulg:
///   Initial params: 493
///   Changed params: 6
///     ts=158196367, name=COM_AUTOS_PAR, value=0
///     ts=158196367, name=MPC_Z_VEL_MAX_DN, value=1.0
///     ts=162054776, name=COM_AUTOS_PAR, value=1
///     ts=162054776, name=MPC_Z_VEL_MAX_DN, value=1.0
///     ts=171616706, name=COM_AUTOS_PAR, value=0
///     ts=176400452, name=COM_AUTOS_PAR, value=1
#[test]
fn test_sample_ulg_parameter_changes() {
    let path = fixture_path("sample.ulg");

    let mut initial_params: Vec<(String, String)> = Vec::new();
    let mut changed_params: Vec<(String, String)> = Vec::new();

    read_file_with_simple_callback(&path, &mut |msg| {
        if let Message::ParameterMessage(pm) = msg {
            let (name, val, stage) = extract_param_info(pm);
            match stage {
                LogStage::Definitions => initial_params.push((name, val)),
                LogStage::Data => changed_params.push((name, val)),
            }
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse sample.ulg");

    // Match pyulog reference: 493 initial parameters.
    assert_eq!(
        initial_params.len(),
        493,
        "Expected 493 initial parameters (pyulog reference), got {}",
        initial_params.len()
    );

    // Match pyulog reference: 6 changed parameters.
    assert_eq!(
        changed_params.len(),
        6,
        "Expected 6 changed parameters (pyulog reference), got {}. Changed: {:?}",
        changed_params.len(),
        changed_params
    );

    // Verify the specific changed parameter names and values match pyulog output.
    assert_eq!(changed_params[0].0, "COM_AUTOS_PAR");
    assert_eq!(changed_params[0].1, "0");

    assert_eq!(changed_params[1].0, "MPC_Z_VEL_MAX_DN");
    assert_eq!(changed_params[1].1, "1");

    assert_eq!(changed_params[2].0, "COM_AUTOS_PAR");
    assert_eq!(changed_params[2].1, "1");

    assert_eq!(changed_params[3].0, "MPC_Z_VEL_MAX_DN");
    assert_eq!(changed_params[3].1, "1");

    assert_eq!(changed_params[4].0, "COM_AUTOS_PAR");
    assert_eq!(changed_params[4].1, "0");

    assert_eq!(changed_params[5].0, "COM_AUTOS_PAR");
    assert_eq!(changed_params[5].1, "1");
}
