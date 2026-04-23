//! Tests against real-world ULog fixtures from mavsim-viewer and pyulog.
//!
//! These exercise the parser against actual flight logs covering different
//! vehicle types, GPS modes, truncation, and appended data.

use px4_ulog::full_parser;
use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, Message, SimpleCallbackResult,
};
fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

// =============================================================================
// Quadrotor (local position, no GPS) — 3.7 MB
// =============================================================================

#[test]
fn test_quadrotor_local_full_parse() {
    let path = fixture_path("quadrotor_local.ulg");
    let parsed = full_parser::read_file(&path).expect("should parse quadrotor log");

    // Should have vehicle_attitude
    assert!(
        parsed.messages.contains_key("vehicle_attitude"),
        "Quadrotor log should have vehicle_attitude"
    );

    // Should have vehicle_local_position (indoor/no GPS)
    assert!(
        parsed.messages.contains_key("vehicle_local_position"),
        "Quadrotor log should have vehicle_local_position"
    );

    // Should have vehicle_status
    assert!(
        parsed.messages.contains_key("vehicle_status"),
        "Quadrotor log should have vehicle_status"
    );
}

#[test]
fn test_quadrotor_local_stream_parse() {
    let path = fixture_path("quadrotor_local.ulg");
    let mut data_count = 0usize;
    let mut log_count = 0usize;
    let mut param_count = 0usize;

    read_file_with_simple_callback(&path, &mut |msg| {
        match msg {
            Message::Data(_) => data_count += 1,
            Message::LoggedMessage(_) => log_count += 1,
            Message::ParameterMessage(_) => param_count += 1,
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            _ => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert!(data_count > 0, "Should have data messages");
    assert!(param_count > 0, "Should have parameter messages");
    eprintln!(
        "quadrotor_local: {} data, {} log, {} param messages",
        data_count, log_count, param_count
    );
}

// =============================================================================
// Fixed-wing with GPS and airspeed — 25 MB
// =============================================================================

#[test]
fn test_fixed_wing_gps_full_parse() {
    let path = fixture_path("fixed_wing_gps.ulg");
    let parsed = full_parser::read_file(&path).expect("should parse fixed-wing log");

    assert!(
        parsed.messages.contains_key("vehicle_attitude"),
        "Fixed-wing should have vehicle_attitude"
    );

    // Fixed-wing with GPS should have global position
    assert!(
        parsed.messages.contains_key("vehicle_global_position"),
        "Fixed-wing should have vehicle_global_position"
    );
}

#[test]
fn test_fixed_wing_gps_stream_parse() {
    let path = fixture_path("fixed_wing_gps.ulg");
    let mut topic_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    read_file_with_simple_callback(&path, &mut |msg| {
        if let Message::Data(data) = msg {
            *topic_counts
                .entry(data.flattened_format.message_name.clone())
                .or_insert(0) += 1;
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    let total: usize = topic_counts.values().sum();
    assert!(
        total > 10000,
        "25MB log should have >10K data messages, got {}",
        total
    );
    eprintln!(
        "fixed_wing_gps: {} total data messages, {} topics",
        total,
        topic_counts.len()
    );
}

// =============================================================================
// VTOL — 16 MB
// =============================================================================

#[test]
fn test_vtol_demo_full_parse() {
    let path = fixture_path("vtol_demo.ulg");
    let parsed = full_parser::read_file(&path).expect("should parse VTOL log");

    assert!(
        parsed.messages.contains_key("vehicle_attitude"),
        "VTOL should have vehicle_attitude"
    );

    // VTOL should have vehicle_status
    assert!(
        parsed.messages.contains_key("vehicle_status"),
        "VTOL should have vehicle_status"
    );
}

#[test]
fn test_vtol_demo_stream_parse() {
    let path = fixture_path("vtol_demo.ulg");
    let mut data_count = 0usize;

    read_file_with_simple_callback(&path, &mut |msg| {
        if let Message::Data(_) = msg {
            data_count += 1;
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert!(data_count > 0, "VTOL log should have data messages");
    eprintln!("vtol_demo: {} data messages", data_count);
}

// =============================================================================
// Truncated/corrupted real log — 6.1 MB
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
