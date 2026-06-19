//! Tests against real-world ULog fixtures from mavsim-viewer and pyulog.
//!
//! These exercise the parser against actual flight logs covering different
//! vehicle types, GPS modes, truncation, and appended data.

use std::collections::BTreeMap;

use px4_ulog::full_parser;
use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, Message, SimpleCallbackResult,
};
use px4_ulog::stream_parser::model::ParameterMessage;
use serde::Serialize;

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

/// A deterministic, snapshottable inventory of a log's contents: data topics with
/// their message counts, parameter values, and logged-message counts per severity.
/// BTreeMaps keep the snapshot output stably ordered.
#[derive(Serialize)]
struct LogSummary {
    topics: BTreeMap<String, usize>,
    parameters: BTreeMap<String, String>,
    logged_messages_by_level: BTreeMap<u8, usize>,
}

/// Build a [`LogSummary`] for a fixture. Captures data topic counts, parameters,
/// and logged-message counts so any drift in the parsed inventory fails the test.
fn log_summary(path: &str) -> LogSummary {
    let mut topics = BTreeMap::new();
    let mut parameters = BTreeMap::new();
    let mut logged_messages_by_level = BTreeMap::new();
    read_file_with_simple_callback(path, &mut |msg| {
        match msg {
            Message::Data(data) => {
                *topics
                    .entry(data.flattened_format.message_name.clone())
                    .or_insert(0) += 1;
            }
            Message::ParameterMessage(param) => match param {
                ParameterMessage::Float(name, value, _) => {
                    parameters.insert(name.to_string(), value.to_string());
                }
                ParameterMessage::Int32(name, value, _) => {
                    parameters.insert(name.to_string(), value.to_string());
                }
            },
            Message::LoggedMessage(logged) => {
                *logged_messages_by_level
                    .entry(logged.log_level)
                    .or_insert(0) += 1;
            }
            _ => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("fixture should parse");
    LogSummary {
        topics,
        parameters,
        logged_messages_by_level,
    }
}

// =============================================================================
// Quadrotor (local position, no GPS), 3.7 MB
// =============================================================================

// The full inventory (topic names + counts, parameters, logged-message counts) is
// snapshotted, so any topic, parameter, or log message that silently appears,
// disappears, or shifts in count fails the test. This subsumes the earlier
// spot-checks for specific topics and the loose count thresholds. Indoor/no-GPS
// quadrotor: expect vehicle_local_position, no vehicle_global_position.
#[test]
fn test_quadrotor_local_topics() {
    insta::assert_yaml_snapshot!(log_summary(&fixture_path("quadrotor_local.ulg")));
}

// =============================================================================
// Fixed-wing with GPS and airspeed, 25 MB
// =============================================================================

// Fixed-wing with GPS: expect vehicle_global_position present. Snapshot the full
// inventory; the exact counts replace the previous ">10K total" threshold.
#[test]
fn test_fixed_wing_gps_topics() {
    insta::assert_yaml_snapshot!(log_summary(&fixture_path("fixed_wing_gps.ulg")));
}

// =============================================================================
// VTOL, 16 MB
// =============================================================================

// VTOL: snapshot the full inventory (topics, parameters, logged messages).
#[test]
fn test_vtol_demo_topics() {
    insta::assert_yaml_snapshot!(log_summary(&fixture_path("vtol_demo.ulg")));
}

// =============================================================================
// Truncated/corrupted real log, 6.1 MB
// =============================================================================

#[test]
fn test_truncated_real_does_not_panic() {
    let path = fixture_path("truncated_real.ulg");
    // This file is known to be truncated. The parser should either:
    // - Parse what it can and return Ok
    // - Return an error
    // But it must NOT panic.
    let result = full_parser::read_file(&path);
    match &result {
        Ok(parsed) => {
            eprintln!(
                "truncated_real: parsed OK with {} topics",
                parsed.messages.len()
            );
        }
        Err(e) => {
            eprintln!("truncated_real: returned error (acceptable): {}", e);
        }
    }
}

#[test]
fn test_truncated_real_stream_does_not_panic() {
    let path = fixture_path("truncated_real.ulg");
    let mut data_count = 0usize;

    let result = read_file_with_simple_callback(&path, &mut |msg| {
        if let Message::Data(_) = msg {
            data_count += 1;
        }
        SimpleCallbackResult::KeepReading
    });

    match result {
        Ok(_) => {
            eprintln!(
                "truncated_real stream: parsed {} data messages before EOF",
                data_count
            );
        }
        Err(e) => {
            eprintln!(
                "truncated_real stream: error after {} data messages: {}",
                data_count, e
            );
        }
    }
    // The key assertion: we got here without panicking
}

// =============================================================================
// Appended data files (from pyulog)
// =============================================================================

#[test]
fn test_appended_data_does_not_panic() {
    let path = fixture_path("sample_appended.ulg");
    let result = full_parser::read_file(&path);
    match &result {
        Ok(parsed) => {
            eprintln!(
                "sample_appended: parsed OK with {} topics",
                parsed.messages.len()
            );
        }
        Err(e) => {
            eprintln!(
                "sample_appended: error (appended data not supported): {}",
                e
            );
        }
    }
}

#[test]
fn test_appended_multiple_does_not_panic() {
    let path = fixture_path("sample_appended_multiple.ulg");
    let result = full_parser::read_file(&path);
    match &result {
        Ok(parsed) => {
            eprintln!(
                "sample_appended_multiple: parsed OK with {} topics",
                parsed.messages.len()
            );
        }
        Err(e) => {
            eprintln!(
                "sample_appended_multiple: error (appended data not supported): {}",
                e
            );
        }
    }
}

// =============================================================================
// All fixtures parse without panic (smoke test)
// =============================================================================

#[test]
fn test_all_fixtures_no_panic() {
    let fixtures = [
        "sample.ulg",
        "6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        "esc_status_log.ulg",
        "quadrotor_local.ulg",
        "fixed_wing_gps.ulg",
        "vtol_demo.ulg",
        "truncated_real.ulg",
        "sample_appended.ulg",
        "sample_appended_multiple.ulg",
    ];

    for name in &fixtures {
        let path = fixture_path(name);
        let result = full_parser::read_file(&path);
        match &result {
            Ok(parsed) => {
                let total_msgs: usize = parsed
                    .messages
                    .values()
                    .flat_map(|m| m.values())
                    .map(|fields| {
                        fields
                            .values()
                            .next()
                            .map(|v| match v {
                                full_parser::SomeVec::UInt64(v) => v.len(),
                                full_parser::SomeVec::Float(v) => v.len(),
                                _ => 0,
                            })
                            .unwrap_or(0)
                    })
                    .sum();
                eprintln!(
                    "  OK: {} ({} topics, ~{} msgs)",
                    name,
                    parsed.messages.len(),
                    total_msgs
                );
            }
            Err(e) => {
                eprintln!("  ERR: {} ({})", name, e);
            }
        }
    }
}
