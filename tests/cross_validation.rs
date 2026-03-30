//! Priority 6: Cross-validation tests against pyulog reference output.
//!
//! Expected values are hard-coded from pyulog output on the same fixture files.
//! This ensures px4-ulog-rs produces identical results to the reference implementation.

use px4_ulog::full_parser;
use px4_ulog::full_parser::SomeVec;
use px4_ulog::stream_parser::file_reader::{read_file_with_simple_callback, Message, SimpleCallbackResult};
use px4_ulog::stream_parser::model::{MultiId, ParameterMessage};

fn somevec_len(v: &SomeVec) -> usize {
    match v {
        SomeVec::Int8(v) => v.len(),
        SomeVec::UInt8(v) => v.len(),
        SomeVec::Int16(v) => v.len(),
        SomeVec::UInt16(v) => v.len(),
        SomeVec::Int32(v) => v.len(),
        SomeVec::UInt32(v) => v.len(),
        SomeVec::Int64(v) => v.len(),
        SomeVec::UInt64(v) => v.len(),
        SomeVec::Float(v) => v.len(),
        SomeVec::Double(v) => v.len(),
        SomeVec::Bool(v) => v.len(),
        SomeVec::Char(v) => v.len(),
    }
}

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

// =============================================================================
// P6-1: Data point counts per topic (from pyulog)
// =============================================================================

/// Reference data from pyulog:
/// ```
/// actuator_controls_0: 3269 msgs, multi_id=0
/// actuator_outputs: 1311 msgs, multi_id=0
/// commander_state: 678 msgs, multi_id=0
/// control_state: 3268 msgs, multi_id=0
/// cpuload: 69 msgs, multi_id=0
/// ekf2_innovations: 3271 msgs, multi_id=0
/// estimator_status: 1311 msgs, multi_id=0
/// sensor_combined: 17070 msgs, multi_id=0
/// sensor_preflight: 17072 msgs, multi_id=0
/// telemetry_status: 70 msgs, multi_id=0
/// vehicle_attitude: 6461 msgs, multi_id=0
/// vehicle_attitude_setpoint: 3272 msgs, multi_id=0
/// vehicle_local_position: 678 msgs, multi_id=0
/// vehicle_rates_setpoint: 6448 msgs, multi_id=0
/// vehicle_status: 294 msgs, multi_id=0
///
/// Total: 64542
/// ```
#[test]
fn test_cross_validate_message_counts_full_parser() {
    let path = fixture_path("sample.ulg");
    let parsed = full_parser::read_file(&path).expect("should parse sample.ulg");

    let expected: Vec<(&str, usize)> = vec![
        ("actuator_controls_0", 3269),
        ("actuator_outputs", 1311),
        ("commander_state", 678),
        ("control_state", 3268),
        ("cpuload", 69),
        ("ekf2_innovations", 3271),
        ("estimator_status", 1311),
        ("sensor_combined", 17070),
        ("sensor_preflight", 17072),
        ("telemetry_status", 70),
        ("vehicle_attitude", 6461),
        ("vehicle_attitude_setpoint", 3272),
        ("vehicle_local_position", 678),
        ("vehicle_rates_setpoint", 6448),
        ("vehicle_status", 294),
    ];

    let mut total_parsed = 0usize;
    for (topic_name, expected_count) in &expected {
        if let Some(multi_map) = parsed.messages.get(*topic_name) {
            if let Some(fields) = multi_map.get(&MultiId::new(0)) {
                // Get count from the "timestamp" field length
                if let Some(ts_vec) = fields.get("timestamp") {
                    let actual_count = somevec_len(ts_vec);
                    total_parsed += actual_count;
                    assert_eq!(
                        actual_count, *expected_count,
                        "Topic '{}': expected {} messages, got {}",
                        topic_name, expected_count, actual_count
                    );
                } else {
                    panic!("Topic '{}' has no 'timestamp' field", topic_name);
                }
            } else {
                panic!("Topic '{}' has no multi_id=0", topic_name);
            }
        } else {
            panic!("Topic '{}' not found in parsed output", topic_name);
        }
    }

    assert_eq!(total_parsed, 64542, "Total data messages should match pyulog");
}

#[test]
fn test_cross_validate_message_counts_stream_parser() {
    let path = fixture_path("sample.ulg");
    let mut topic_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    read_file_with_simple_callback(&path, &mut |msg| {
        if let Message::Data(data_msg) = msg {
            *topic_counts
                .entry(data_msg.flattened_format.message_name.clone())
                .or_insert(0) += 1;
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse sample.ulg");

    // Verify total data messages
    let total: usize = topic_counts.values().sum();
    assert!(total > 60000, "Expected >60K data messages, got {}", total);

    // Verify key topics are present
    assert!(topic_counts.contains_key("sensor_combined"), "sensor_combined should be present");
    assert!(topic_counts.contains_key("vehicle_attitude"), "vehicle_attitude should be present");
}

// =============================================================================
// P6-2: Parameter validation
// =============================================================================

#[test]
fn test_cross_validate_parameters() {
    let path = fixture_path("sample.ulg");
    let mut params: Vec<(String, String)> = Vec::new();

    read_file_with_simple_callback(&path, &mut |msg| {
        if let Message::ParameterMessage(param) = msg {
            match param {
                ParameterMessage::Int32(name, val, _) => {
                    params.push((name.to_string(), format!("{}", val)));
                }
                ParameterMessage::Float(name, val, _) => {
                    params.push((name.to_string(), format!("{}", val)));
                }
            }
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    // pyulog reports 493 initial parameters + 6 changed = up to 499
    // The stream parser collects both initial and changed params
    assert!(
        params.len() >= 400,
        "Expected at least 400 parameters, got {}",
        params.len()
    );

    // Spot-check specific known parameters (from pyulog)
    let param_map: std::collections::HashMap<_, _> = params.into_iter().collect();
    assert!(
        param_map.contains_key("ATT_ACC_COMP"),
        "ATT_ACC_COMP parameter should exist"
    );
}

// =============================================================================
// P6-3: Logged string messages
// =============================================================================

#[test]
fn test_cross_validate_logged_messages() {
    let path = fixture_path("sample.ulg");
    let mut messages = Vec::new();

    read_file_with_simple_callback(&path, &mut |msg| {
        if let Message::LoggedMessage(log_msg) = msg {
            messages.push((
                log_msg.log_level,
                log_msg.timestamp,
                log_msg.logged_message.to_string(),
            ));
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    // pyulog reports 4 logged messages, all about barometer:
    // level=51, msg="[sensors] no barometer found on /dev/baro0 (2)"
    assert_eq!(messages.len(), 4, "Expected 4 logged messages");
    for (level, _ts, msg) in &messages {
        assert_eq!(*level, 51, "Log level should be 51 (0x33)");
        assert!(
            msg.contains("barometer"),
            "Message should mention barometer: {}",
            msg
        );
    }
}

// =============================================================================
// P6-4: Start timestamp
// =============================================================================

#[test]
fn test_cross_validate_start_timestamp() {
    // pyulog reports start_timestamp: 112500176
    use px4_ulog::stream_parser::LogParser;

    let path = fixture_path("sample.ulg");
    let data = std::fs::read(&path).unwrap();
    let mut parser = LogParser::default();
    parser.consume_bytes(&data).unwrap();
    assert_eq!(parser.timestamp(), 112500176, "Start timestamp should match pyulog");
}

// =============================================================================
// P6-5: Topic list from seek parser
// =============================================================================

#[test]
fn test_cross_validate_topic_list() {
    let path = fixture_path("sample.ulg");
    let parsed = full_parser::read_file(&path).expect("should parse sample.ulg");
    let names: Vec<&String> = parsed.messages.keys().collect();

    // full_parser only includes topics with actual data messages
    let expected_topics = [
        "sensor_combined",
        "vehicle_attitude",
        "estimator_status",
    ];
    for topic in &expected_topics {
        assert!(
            names.iter().any(|n| n.as_str() == *topic),
            "Topic '{}' should be in parsed output",
            topic
        );
    }
}

// =============================================================================
// P6-6: GPS data values (seek parser cross-check)
// =============================================================================

#[test]
fn test_cross_validate_gps_data_count() {
    let path = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let parsed = full_parser::read_file(&path).expect("should parse");
    let gps = parsed
        .messages
        .get("vehicle_gps_position")
        .and_then(|m| m.get(&MultiId::new(0)))
        .expect("should have GPS data");
    let count = somevec_len(gps.get("timestamp").expect("should have timestamp"));
    assert_eq!(count, 260, "GPS position count should match");
}

// =============================================================================
// P6-7: ESC status nested message
// =============================================================================

#[test]
fn test_cross_validate_esc_status() {
    let path = fixture_path("esc_status_log.ulg");
    let parsed = full_parser::read_file(&path).expect("should parse esc_status_log.ulg");

    assert!(
        parsed.messages.contains_key("esc_status"),
        "esc_status topic should be present"
    );
    let esc = parsed.messages.get("esc_status").unwrap();
    let fields = esc.get(&MultiId::new(0)).unwrap();

    // ESC status should have nested fields like esc[0].esc_rpm
    let has_nested = fields.keys().any(|k| k.contains("esc[") && k.contains("].esc_"));
    assert!(
        has_nested,
        "esc_status should have nested fields like esc[N].esc_rpm, found: {:?}",
        fields.keys().take(10).collect::<Vec<_>>()
    );
}
