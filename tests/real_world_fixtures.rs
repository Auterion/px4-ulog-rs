//! Tests against real-world ULog fixtures from mavsim-viewer and pyulog.
//!
//! These exercise the parser against actual flight logs covering different
//! vehicle types, GPS modes, truncation, and appended data.

use std::collections::BTreeMap;

use px4_ulog::full_parser;
use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, Message, SimpleCallbackResult,
};
fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

/// Map every data topic in a log to its message count. Structural only: topic
/// names and counts are independent of the runtime GPS offset applied to scrubbed
/// fixtures, so this is deterministic and safe to snapshot (no GPS values leak in).
/// A BTreeMap keeps the snapshot output stably ordered.
fn topic_summary(path: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    read_file_with_simple_callback(path, &mut |msg| {
        if let Message::Data(data) = msg {
            *counts
                .entry(data.flattened_format.message_name.clone())
                .or_insert(0) += 1;
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("fixture should parse");
    counts
}

// =============================================================================
// Quadrotor (local position, no GPS), 3.7 MB
// =============================================================================

// The topic inventory (names + exact per-topic data counts) is snapshotted, so
// any topic that silently appears, disappears, or shifts in count fails the test.
// This subsumes the earlier spot-checks for specific topics and the loose count
// thresholds. Indoor/no-GPS quadrotor: expect vehicle_local_position, no
// vehicle_global_position.
#[test]
fn test_quadrotor_local_topics() {
    insta::assert_yaml_snapshot!(topic_summary(&fixture_path("quadrotor_local.ulg")));
}

// =============================================================================
// Fixed-wing with GPS and airspeed, 25 MB
// =============================================================================

// Fixed-wing with GPS: expect vehicle_global_position present. Snapshot the full
// inventory; the exact counts replace the previous ">10K total" threshold.
#[test]
fn test_fixed_wing_gps_topics() {
    insta::assert_yaml_snapshot!(topic_summary(&fixture_path("fixed_wing_gps.ulg")));
}

// =============================================================================
// VTOL, 16 MB
// =============================================================================

// VTOL: snapshot the full topic inventory and counts.
#[test]
fn test_vtol_demo_topics() {
    insta::assert_yaml_snapshot!(topic_summary(&fixture_path("vtol_demo.ulg")));
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
