//! Smoke tests for every non-data ULog message type the streaming parser
//! surfaces to callers. Each variant gets one synthetic stream built via
//! `ULogBuilder` plus, where applicable, one real-fixture check against
//! reference values from pyulog.
//!
//! MultiInfo fragment reassembly is covered by tests/multi_info_reassembly.rs
//! and RemoveLogged semantics by tests/remove_logged_semantics.rs, so those
//! variants are only exercised here through the combined-stream integration
//! test.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, Message, SimpleCallbackResult,
};
use px4_ulog::stream_parser::model;

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

/// Write `bytes` to a temp file, parse it, collect values the caller
/// extracts from each `Message`, and clean up. Keeps the message-dispatch
/// boilerplate out of every test.
fn collect_from_bytes<T>(
    bytes: &[u8],
    test_name: &str,
    mut extract: impl FnMut(&Message) -> Option<T>,
) -> Vec<T> {
    let tmp = std::env::temp_dir().join(format!("{}.ulg", test_name));
    std::fs::write(&tmp, bytes).expect("write temp file");
    let out = collect_from_path(tmp.to_str().unwrap(), &mut extract);
    let _ = std::fs::remove_file(&tmp);
    out
}

fn collect_from_path<T>(
    path: &str,
    extract: &mut dyn FnMut(&Message) -> Option<T>,
) -> Vec<T> {
    let mut out = Vec::new();
    read_file_with_simple_callback(path, &mut |msg| {
        if let Some(v) = extract(msg) {
            out.push(v);
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("parser should not error on valid input");
    out
}

// =============================================================================
// Info ('I')
// =============================================================================

#[test]
fn info_synthetic_multiple_keys_are_surfaced() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.info("char", "sys_name", b"PX4");
    builder.info("char", "ver_hw", b"AUAV_X21");
    builder.info("int32_t", "ver_sw_release", &42i32.to_le_bytes());

    let infos = collect_from_bytes(&builder.build(), "info_multi_keys", |msg| {
        if let Message::InfoMessage(i) = msg {
            Some((i.key.to_string(), i.value.to_vec()))
        } else {
            None
        }
    });

    let keys: Vec<_> = infos.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, vec!["sys_name", "ver_hw", "ver_sw_release"]);
    assert_eq!(infos[0].1, b"PX4");
    assert_eq!(infos[1].1, b"AUAV_X21");
    assert_eq!(infos[2].1, 42i32.to_le_bytes());
}

/// Reference values extracted from pyulog for sample.ulg.
#[test]
fn info_sample_ulg_matches_pyulog_reference() {
    let mut sys_name = None;
    let mut ver_hw = None;
    let mut ver_sw = None;

    read_file_with_simple_callback(&fixture_path("sample.ulg"), &mut |msg| {
        if let Message::InfoMessage(i) = msg {
            let val = String::from_utf8_lossy(i.value).to_string();
            match i.key {
                "sys_name" => sys_name = Some(val),
                "ver_hw" => ver_hw = Some(val),
                "ver_sw" => ver_sw = Some(val),
                _ => {}
            }
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("parse sample.ulg");

    assert_eq!(sys_name.as_deref(), Some("PX4"));
    assert_eq!(ver_hw.as_deref(), Some("AUAV_X21"));
    assert_eq!(
        ver_sw.as_deref(),
        Some("fd483321a5cf50ead91164356d15aa474643aa73")
    );
}

// =============================================================================
// MultiInfo ('M')
// =============================================================================

/// Minimal coverage: verify the raw MultiInfo variant is surfaced. Fragment
/// reassembly is exercised in tests/multi_info_reassembly.rs.
#[test]
fn multi_info_synthetic_fragment_is_surfaced() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();

    let key = b"char[4] replay";
    let mut payload = vec![0u8, key.len() as u8]; // is_continued=false, key_len
    payload.extend_from_slice(key);
    payload.extend_from_slice(b"test");
    builder.unknown_message(b'M', &payload);

    let fragments = collect_from_bytes(&builder.build(), "multi_info_fragment", |msg| {
        if let Message::MultiInfoMessage(mi) = msg {
            Some((mi.is_continued, mi.key.to_string()))
        } else {
            None
        }
    });

    assert_eq!(fragments, vec![(false, "replay".to_string())]);
}

// =============================================================================
// Dropout ('O')
// =============================================================================

#[test]
fn dropout_synthetic_duration_is_surfaced() {
    let (mut builder, _) = ULogBuilder::minimal_with_data();
    builder.dropout(150);

    let durations = collect_from_bytes(&builder.build(), "dropout_synth", |msg| {
        if let Message::DropoutMessage(d) = msg { Some(d.duration_ms) } else { None }
    });

    assert_eq!(durations, vec![150]);
}

/// Reference values from pyulog for sample.ulg: 4 dropouts, durations [0, 26, 31, 62] ms.
#[test]
fn dropout_sample_ulg_matches_pyulog_reference() {
    let durations = collect_from_path(&fixture_path("sample.ulg"), &mut |msg| {
        if let Message::DropoutMessage(d) = msg { Some(d.duration_ms) } else { None }
    });
    assert_eq!(durations, vec![0, 26, 31, 62]);
}

// =============================================================================
// Sync ('S')
// =============================================================================

#[test]
fn sync_synthetic_exposes_spec_magic_bytes() {
    let (mut builder, _) = ULogBuilder::minimal_with_data();
    builder.sync();

    let magic = collect_from_bytes(&builder.build(), "sync_magic", |msg| {
        if let Message::SyncMessage(s) = msg { Some(s.magic) } else { None }
    });

    assert_eq!(
        magic,
        vec![[0x2F, 0x73, 0x13, 0x20, 0x25, 0x0C, 0xBB, 0x12]]
    );
}

// =============================================================================
// Tagged Logged String ('C')
// =============================================================================

#[test]
fn tagged_logged_string_fields_round_trip() {
    let (mut builder, _) = ULogBuilder::minimal_with_data();
    builder.tagged_logged_string(6, 42, 5_000_000, "Tagged test message");

    let entries = collect_from_bytes(&builder.build(), "tagged_logged", |msg| {
        if let Message::TaggedLoggedMessage(t) = msg {
            Some((t.log_level, t.tag, t.timestamp, t.logged_message.to_string()))
        } else {
            None
        }
    });

    assert_eq!(
        entries,
        vec![(6, 42, 5_000_000, "Tagged test message".to_string())]
    );
}

// =============================================================================
// Parameter Default ('Q')
// =============================================================================

#[test]
fn parameter_default_surfaces_system_and_current_config_flags() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.parameter_default_i32(0x01, "SYS_AUTOSTART", 4001);
    builder.parameter_default_i32(0x02, "MC_PITCHRATE_P", 150);

    let defaults = collect_from_bytes(&builder.build(), "param_default", |msg| {
        if let Message::ParameterDefaultMessage(model::ParameterDefaultMessage::Int32(name, val, flags)) = msg {
            Some((*flags, name.to_string(), *val))
        } else {
            None
        }
    });

    assert_eq!(
        defaults,
        vec![
            (0x01, "SYS_AUTOSTART".to_string(), 4001),
            (0x02, "MC_PITCHRATE_P".to_string(), 150),
        ]
    );
}

// =============================================================================
// Integration: every message type coexists in one stream without corruption
// =============================================================================

#[test]
fn mixed_stream_surfaces_every_variant_and_preserves_data() {
    let (mut builder, msg_id) = ULogBuilder::minimal_with_data();

    builder.info("char", "test_key", b"test_value");
    builder.dropout(100);
    builder.sync();
    builder.remove_logged(msg_id);
    builder.tagged_logged_string(6, 1, 9999, "tagged msg");
    builder.parameter_default_i32(0x01, "TEST_PARAM", 42);

    // Second data message to confirm data delivery survives the injected types.
    let mut payload = Vec::new();
    payload.extend_from_slice(&2000u64.to_le_bytes());
    payload.extend_from_slice(&2.5f32.to_le_bytes());
    builder.data(msg_id, &payload);

    let tmp = std::env::temp_dir().join("mixed_stream.ulg");
    std::fs::write(&tmp, builder.build()).unwrap();

    let mut counts = std::collections::HashMap::<&'static str, usize>::new();
    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        let tag = match msg {
            Message::Data(_) => "data",
            Message::InfoMessage(_) => "info",
            Message::DropoutMessage(_) => "dropout",
            Message::SyncMessage(_) => "sync",
            Message::RemoveLoggedMessage(_) => "remove",
            Message::TaggedLoggedMessage(_) => "tagged",
            Message::ParameterDefaultMessage(_) => "param_default",
            Message::LoggedMessage(_)
            | Message::ParameterMessage(_)
            | Message::MultiInfoMessage(_) => return SimpleCallbackResult::KeepReading,
        };
        *counts.entry(tag).or_insert(0) += 1;
        SimpleCallbackResult::KeepReading
    })
    .expect("mixed stream should parse cleanly");

    let _ = std::fs::remove_file(&tmp);

    assert_eq!(counts.get("data"), Some(&2), "both data messages must survive injected types");
    assert_eq!(counts.get("info"), Some(&1));
    assert_eq!(counts.get("dropout"), Some(&1));
    assert_eq!(counts.get("sync"), Some(&1));
    assert_eq!(counts.get("remove"), Some(&1));
    assert_eq!(counts.get("tagged"), Some(&1));
    assert_eq!(counts.get("param_default"), Some(&1));
}
