//! Test-driven development tests for ULog message types currently ignored by the streaming parser.
//!
//! The streaming parser (stream_parser/file_reader.rs) silently discards these message types
//! at line ~390 via `_ => ()`:
//!
//!   - Info (I)              -- key-value metadata
//!   - MultiInfo (M)         -- continued key-value data
//!   - Dropout (O)           -- data loss markers
//!   - Sync (S)              -- corruption recovery markers
//!   - Remove Logged Msg (R) -- topic unsubscription
//!   - Tagged Logged Str (C) -- tagged log messages
//!   - Parameter Default (Q) -- default parameter values
//!
//! These tests define the EXPECTED behavior and will FAIL until the features are implemented.
//! That is intentional -- this is TDD.
//!
//! Implementation checklist:
//!   1. Add new variants to `Message` enum in file_reader.rs:
//!        InfoMessage(&'a InfoMessage<'a>)
//!        MultiInfoMessage(&'a MultiInfoMessage<'a>)
//!        DropoutMessage(&'a DropoutMessage)
//!        SyncMessage(&'a SyncMessage)
//!        RemoveLoggedMessage(&'a RemoveLoggedMsg)
//!        TaggedLoggedMessage(&'a TaggedLoggedStringMessage<'a>)
//!        ParameterDefaultMessage(&'a ParameterDefaultMessage<'a>)
//!
//!   2. Add corresponding structs to model.rs.
//!
//!   3. Add callbacks to LogParser and wire them through `parse_message`.
//!
//!   4. Wire the new callbacks through `read_file_with_simple_callback`.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, LogParser, Message, SimpleCallbackResult,
};
use px4_ulog::stream_parser::model;

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

/// Helper: parse raw bytes through LogParser with all callbacks, collecting Messages.
/// This uses consume_bytes directly so we can test synthetic byte streams without files.
#[allow(dead_code)]
fn parse_bytes_collecting(bytes: &[u8]) -> Vec<String> {
    // We collect a string tag for each message received so we can assert on message types.
    // Once the Message enum has new variants, the match arms below will compile.
    let collected = std::cell::RefCell::new(Vec::new());

    let mut data_cb = |_: &model::DataMessage| {
        collected.borrow_mut().push("Data".to_string());
    };
    let mut log_cb = |_: &model::LoggedStringMessage| {
        collected.borrow_mut().push("LoggedMessage".to_string());
    };
    let mut param_cb = |_: &model::ParameterMessage| {
        collected.borrow_mut().push("ParameterMessage".to_string());
    };

    // TODO: Once new callbacks are added to LogParser, register them here:
    //   let mut info_cb = |msg: &model::InfoMessage| { ... };
    //   parser.set_info_message_callback(&mut info_cb);
    //   (and similarly for multi_info, dropout, sync, remove_logged, tagged_logged, param_default)

    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut data_cb);
    parser.set_logged_string_message_callback(&mut log_cb);
    parser.set_parameter_message_callback(&mut param_cb);

    parser
        .consume_bytes(bytes)
        .expect("parse should not error on valid input");

    collected.into_inner()
}

// =============================================================================
// Info Message ('I') — key-value metadata
// =============================================================================

/// Info messages carry key-value pairs like sys_name, ver_hw, ver_sw.
/// The parser should surface these through a new `Message::InfoMessage` variant.
///
/// ULog Info message format:
///   key_len (u8) | key (key_len bytes, formatted as "type[size] name") | value (remaining bytes)
#[test]
fn test_info_message_synthetic_string_value() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.info("char", "sys_name", b"TestSystem");
    let bytes = builder.build();

    // Parse and collect messages via the simple callback API.
    // This requires writing to a temp file since read_file_with_simple_callback reads from disk.
    let tmp = std::env::temp_dir().join("test_info_synthetic.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut info_messages: Vec<(String, Vec<u8>)> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        // TODO: Once Message::InfoMessage variant exists, uncomment and use:
        // if let Message::InfoMessage(info) = msg {
        //     info_messages.push((info.key.to_string(), info.value.to_vec()));
        // }

        // For now, we check that the message is NOT silently dropped.
        // This match must be exhaustive -- if InfoMessage variant is added, this will
        // need updating (which is the point).
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(info) => {
                info_messages.push((info.key.to_string(), info.value.to_vec()));
            }
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    // EXPECTED: The info message should have been received, not silently dropped.
    // This assertion will fail until InfoMessage support is implemented.
    assert!(
        !info_messages.is_empty(),
        "Info message ('I') was silently dropped. \
         The parser needs a new Message::InfoMessage variant and callback. \
         Expected key='sys_name', value=b'TestSystem'."
    );
    assert_eq!(info_messages[0].0, "sys_name");
    assert_eq!(info_messages[0].1, b"TestSystem");

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_info_message_multiple_keys() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.info("char", "sys_name", b"PX4");
    builder.info("char", "ver_hw", b"AUAV_X21");
    builder.info("int32_t", "ver_sw_release", &42i32.to_le_bytes());
    let bytes = builder.build();

    let tmp = std::env::temp_dir().join("test_info_multi_keys.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut info_keys: Vec<String> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(info) => {
                info_keys.push(info.key.to_string());
            }
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    // EXPECTED: All three info messages should be received.
    assert_eq!(
        info_keys.len(),
        3,
        "Expected 3 info messages, got {}. Info messages are silently dropped.",
        info_keys.len()
    );
    assert_eq!(info_keys[0], "sys_name");
    assert_eq!(info_keys[1], "ver_hw");
    assert_eq!(info_keys[2], "ver_sw_release");

    let _ = std::fs::remove_file(&tmp);
}

/// Verify Info messages from the real sample.ulg match pyulog reference output.
/// pyulog reports:
///   sys_name: PX4
///   ver_hw: AUAV_X21
///   ver_sw: fd483321a5cf50ead91164356d15aa474643aa73
#[test]
fn test_info_message_sample_ulg_sys_name() {
    let path = fixture_path("sample.ulg");

    let mut sys_name: Option<String> = None;
    let mut ver_hw: Option<String> = None;
    let mut ver_sw: Option<String> = None;

    read_file_with_simple_callback(&path, &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(info) => {
                let val_str = String::from_utf8_lossy(info.value).to_string();
                match info.key {
                    "sys_name" => sys_name = Some(val_str),
                    "ver_hw" => ver_hw = Some(val_str),
                    "ver_sw" => ver_sw = Some(val_str),
                    _ => {}
                }
            }
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse sample.ulg");

    // EXPECTED values from pyulog:
    assert_eq!(
        sys_name.as_deref(),
        Some("PX4"),
        "Info sys_name should be 'PX4'. Info messages are silently dropped."
    );
    assert_eq!(
        ver_hw.as_deref(),
        Some("AUAV_X21"),
        "Info ver_hw should be 'AUAV_X21'. Info messages are silently dropped."
    );
    assert_eq!(
        ver_sw.as_deref(),
        Some("fd483321a5cf50ead91164356d15aa474643aa73"),
        "Info ver_sw should be 'fd483321a5cf50ead91164356d15aa474643aa73'. \
         Info messages are silently dropped."
    );
}

// =============================================================================
// MultiInfo Message ('M') — continued key-value data
// =============================================================================

/// MultiInfo messages extend Info messages for values that exceed a single message size.
/// They carry an `is_continued` flag and the same key-value structure.
///
/// ULog MultiInfo format:
///   is_continued (u8) | key_len (u8) | key (key_len bytes) | value (remaining)
#[test]
fn test_multi_info_message_synthetic() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();

    // Simulate a multi-info message: is_continued=0, key="char[4] replay", value="test"
    let key = b"char[4] replay";
    let value = b"test";
    let mut payload = Vec::new();
    payload.push(0u8); // is_continued = false (first fragment)
    payload.push(key.len() as u8);
    payload.extend_from_slice(key);
    payload.extend_from_slice(value);
    builder.unknown_message(b'M', &payload);

    let bytes = builder.build();
    let tmp = std::env::temp_dir().join("test_multi_info_synthetic.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut multi_info_count = 0usize;

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(mi) => {
                multi_info_count += 1;
                assert_eq!(mi.is_continued, false);
                assert_eq!(mi.key, "replay");
            }
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(
        multi_info_count, 1,
        "MultiInfo message ('M') was silently dropped. \
         The parser needs a new Message::MultiInfoMessage variant."
    );

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_multi_info_continued_fragments() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();

    // First fragment: is_continued=0
    let key = b"char[8] long_val";
    let mut payload1 = Vec::new();
    payload1.push(0u8); // is_continued = false
    payload1.push(key.len() as u8);
    payload1.extend_from_slice(key);
    payload1.extend_from_slice(b"AAAABBBB");
    builder.unknown_message(b'M', &payload1);

    // Second fragment: is_continued=1
    let mut payload2 = Vec::new();
    payload2.push(1u8); // is_continued = true
    payload2.push(key.len() as u8);
    payload2.extend_from_slice(key);
    payload2.extend_from_slice(b"CCCCDDDD");
    builder.unknown_message(b'M', &payload2);

    let bytes = builder.build();
    let tmp = std::env::temp_dir().join("test_multi_info_continued.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut fragment_count = 0usize;

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {
                fragment_count += 1;
            }
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(
        fragment_count, 2,
        "Expected 2 MultiInfo fragments (first + continuation), got {}. \
         MultiInfo messages are silently dropped.",
        fragment_count
    );

    let _ = std::fs::remove_file(&tmp);
}

// =============================================================================
// Dropout Message ('O') — data loss markers
// =============================================================================

/// Dropout messages indicate data loss, carrying a u16 duration in milliseconds.
/// The parser should surface these through a new `Message::DropoutMessage` variant.
///
/// ULog Dropout format:
///   duration_ms (u16 LE)
#[test]
fn test_dropout_message_synthetic() {
    let (mut builder, _msg_id) = ULogBuilder::minimal_with_data();
    builder.dropout(150);
    let bytes = builder.build();

    let tmp = std::env::temp_dir().join("test_dropout_synthetic.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut dropout_durations: Vec<u16> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(dropout) => {
                dropout_durations.push(dropout.duration_ms);
            }
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(
        dropout_durations.len(),
        1,
        "Dropout message ('O') was silently dropped. \
         The parser needs a new Message::DropoutMessage variant."
    );
    assert_eq!(
        dropout_durations[0], 150,
        "Dropout duration should be 150ms"
    );

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_dropout_message_zero_duration() {
    let (mut builder, _msg_id) = ULogBuilder::minimal_with_data();
    builder.dropout(0);
    let bytes = builder.build();

    let tmp = std::env::temp_dir().join("test_dropout_zero.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut dropout_durations: Vec<u16> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(dropout) => {
                dropout_durations.push(dropout.duration_ms);
            }
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(
        dropout_durations.len(),
        1,
        "Dropout message with 0ms duration was silently dropped."
    );
    assert_eq!(dropout_durations[0], 0, "Dropout duration should be 0ms");

    let _ = std::fs::remove_file(&tmp);
}

/// Verify dropouts from the real sample.ulg match pyulog reference output.
/// pyulog reports 4 dropouts with durations: 0ms, 26ms, 31ms, 62ms.
#[test]
fn test_dropout_message_sample_ulg() {
    let path = fixture_path("sample.ulg");

    let mut dropout_durations: Vec<u16> = Vec::new();

    read_file_with_simple_callback(&path, &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(dropout) => {
                dropout_durations.push(dropout.duration_ms);
            }
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse sample.ulg");

    // EXPECTED values from pyulog:
    assert_eq!(
        dropout_durations.len(),
        4,
        "Expected 4 dropouts in sample.ulg (pyulog reference), got {}. \
         Dropout messages are silently dropped.",
        dropout_durations.len()
    );
    assert_eq!(
        dropout_durations,
        vec![0, 26, 31, 62],
        "Dropout durations should match pyulog output: [0, 26, 31, 62] ms"
    );
}

// =============================================================================
// Sync Message ('S') — corruption recovery markers
// =============================================================================

/// Sync messages contain 8 magic bytes used to recover from corruption.
/// The parser should surface these through a new `Message::SyncMessage` variant.
///
/// ULog Sync format:
///   magic[8] = [0x2F, 0x73, 0x13, 0x20, 0x25, 0x0C, 0xBB, 0x12]
#[test]
fn test_sync_message_synthetic() {
    let (mut builder, _msg_id) = ULogBuilder::minimal_with_data();
    builder.sync();
    let bytes = builder.build();

    let tmp = std::env::temp_dir().join("test_sync_synthetic.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut sync_count = 0usize;

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {
                sync_count += 1;
            }
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(
        sync_count, 1,
        "Sync message ('S') was silently dropped. \
         The parser needs a new Message::SyncMessage variant."
    );

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_sync_message_magic_bytes_verified() {
    let (mut builder, _msg_id) = ULogBuilder::minimal_with_data();
    builder.sync();
    let bytes = builder.build();

    let tmp = std::env::temp_dir().join("test_sync_magic.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut received_magic: Option<[u8; 8]> = None;

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(sync) => {
                received_magic = Some(sync.magic);
            }
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    let expected_magic: [u8; 8] = [0x2F, 0x73, 0x13, 0x20, 0x25, 0x0C, 0xBB, 0x12];
    assert_eq!(
        received_magic,
        Some(expected_magic),
        "Sync message magic bytes should be [0x2F, 0x73, 0x13, 0x20, 0x25, 0x0C, 0xBB, 0x12]. \
         Sync messages are silently dropped."
    );

    let _ = std::fs::remove_file(&tmp);
}

// =============================================================================
// Remove Logged Message ('R') — topic unsubscription
// =============================================================================

/// Remove Logged Message tells the parser a topic subscription has been removed.
/// The parser should surface these through a new `Message::RemoveLoggedMessage` variant.
///
/// ULog RemoveLoggedMessage format:
///   msg_id (u16 LE)
#[test]
fn test_remove_logged_message_synthetic() {
    let (mut builder, msg_id) = ULogBuilder::minimal_with_data();
    builder.remove_logged(msg_id);
    let bytes = builder.build();

    let tmp = std::env::temp_dir().join("test_remove_logged.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut removed_ids: Vec<u16> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(rm) => {
                removed_ids.push(rm.msg_id);
            }
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(
        removed_ids.len(),
        1,
        "RemoveLoggedMessage ('R') was silently dropped. \
         The parser needs a new Message::RemoveLoggedMessage variant."
    );
    assert_eq!(
        removed_ids[0], msg_id,
        "Removed msg_id should match the subscribed topic"
    );

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_remove_logged_message_nonexistent_id() {
    // Removing a msg_id that was never subscribed should still be reported,
    // even if semantically a no-op. The parser should not error.
    let (mut builder, _msg_id) = ULogBuilder::minimal_with_data();
    builder.remove_logged(9999); // never subscribed
    let bytes = builder.build();

    let tmp = std::env::temp_dir().join("test_remove_logged_nonexistent.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut removed_ids: Vec<u16> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(rm) => {
                removed_ids.push(rm.msg_id);
            }
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse without error even for non-existent msg_id");

    assert_eq!(
        removed_ids.len(),
        1,
        "RemoveLoggedMessage for non-existent msg_id should still be reported."
    );
    assert_eq!(removed_ids[0], 9999);

    let _ = std::fs::remove_file(&tmp);
}

// =============================================================================
// Tagged Logged String ('C') — tagged log messages
// =============================================================================

/// Tagged logged strings are like logged strings ('L') but with an additional u16 tag field.
/// The parser should surface these through a new `Message::TaggedLoggedMessage` variant.
///
/// ULog TaggedLoggedString format:
///   log_level (u8) | tag (u16 LE) | timestamp (u64 LE) | message (remaining bytes)
#[test]
fn test_tagged_logged_string_synthetic() {
    let (mut builder, _msg_id) = ULogBuilder::minimal_with_data();
    builder.tagged_logged_string(6, 42, 5000000, "Tagged test message");
    let bytes = builder.build();

    let tmp = std::env::temp_dir().join("test_tagged_logged.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut tagged_messages: Vec<(u8, u16, u64, String)> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(tlm) => {
                tagged_messages.push((
                    tlm.log_level,
                    tlm.tag,
                    tlm.timestamp,
                    tlm.logged_message.to_string(),
                ));
            }
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(
        tagged_messages.len(),
        1,
        "TaggedLoggedString ('C') was silently dropped. \
         The parser needs a new Message::TaggedLoggedMessage variant."
    );
    let (level, tag, ts, text) = &tagged_messages[0];
    assert_eq!(*level, 6, "log_level should be 6 (INFO)");
    assert_eq!(*tag, 42, "tag should be 42");
    assert_eq!(*ts, 5000000, "timestamp should be 5000000");
    assert_eq!(text, "Tagged test message");

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_tagged_logged_string_different_levels() {
    let (mut builder, _msg_id) = ULogBuilder::minimal_with_data();
    builder.tagged_logged_string(3, 1, 1000, "Error msg"); // ERROR
    builder.tagged_logged_string(4, 2, 2000, "Warning msg"); // WARNING
    builder.tagged_logged_string(7, 3, 3000, "Debug msg"); // DEBUG
    let bytes = builder.build();

    let tmp = std::env::temp_dir().join("test_tagged_logged_levels.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut tagged_levels: Vec<u8> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(tlm) => {
                tagged_levels.push(tlm.log_level);
            }
            Message::ParameterDefaultMessage(_) => {}
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(
        tagged_levels.len(),
        3,
        "Expected 3 tagged logged strings, got {}. Tagged messages are silently dropped.",
        tagged_levels.len()
    );
    assert_eq!(tagged_levels, vec![3, 4, 7]);

    let _ = std::fs::remove_file(&tmp);
}

// =============================================================================
// Parameter Default ('Q') — default parameter values
// =============================================================================

/// Parameter Default messages carry the system/firmware default value for a parameter,
/// along with a `default_types` bitfield indicating which defaults apply.
///
/// ULog ParameterDefault format:
///   default_types (u8) | key_len (u8) | key (key_len bytes, "type name") | value (remaining)
///
/// default_types bits:
///   1 (0x01) = system-wide default
///   2 (0x02) = current-configuration default
#[test]
fn test_parameter_default_i32_synthetic() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    builder.parameter_default_i32(0x01, "SYS_AUTOSTART", 4001);
    let bytes = builder.build();

    let tmp = std::env::temp_dir().join("test_param_default_i32.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut param_defaults: Vec<(u8, String, i32)> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(pd) => {
                if let model::ParameterDefaultMessage::Int32(name, val, default_types) = pd {
                    param_defaults.push((*default_types, name.to_string(), *val));
                }
            }
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(
        param_defaults.len(),
        1,
        "ParameterDefault ('Q') was silently dropped. \
         The parser needs a new Message::ParameterDefaultMessage variant."
    );
    let (default_types, name, value) = &param_defaults[0];
    assert_eq!(*default_types, 0x01, "default_types should be 0x01 (system default)");
    assert_eq!(name, "SYS_AUTOSTART");
    assert_eq!(*value, 4001);

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn test_parameter_default_current_config() {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    // 0x02 = current-configuration default
    builder.parameter_default_i32(0x02, "MC_PITCHRATE_P", 150);
    let bytes = builder.build();

    let tmp = std::env::temp_dir().join("test_param_default_config.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut param_defaults: Vec<(u8, String)> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => {}
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => {}
            Message::DropoutMessage(_) => {}
            Message::SyncMessage(_) => {}
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => {}
            Message::TaggedLoggedMessage(_) => {}
            Message::ParameterDefaultMessage(pd) => {
                if let model::ParameterDefaultMessage::Int32(name, _, default_types) = pd {
                    param_defaults.push((*default_types, name.to_string()));
                }
            }
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse");

    assert_eq!(
        param_defaults.len(),
        1,
        "ParameterDefault with current-config type was silently dropped."
    );
    assert_eq!(param_defaults[0].0, 0x02, "default_types should be 0x02 (current config)");
    assert_eq!(param_defaults[0].1, "MC_PITCHRATE_P");

    let _ = std::fs::remove_file(&tmp);
}

// =============================================================================
// Integration: multiple ignored types in a single stream
// =============================================================================

/// A ULog stream can contain a mix of all message types. Verify that none of the
/// currently-ignored types cause parsing errors or data corruption in adjacent messages.
#[test]
fn test_all_ignored_types_in_one_stream() {
    let (mut builder, msg_id) = ULogBuilder::minimal_with_data();

    // Inject every currently-ignored message type into the data section
    builder.info("char", "test_key", b"test_value");
    builder.dropout(100);
    builder.sync();
    builder.remove_logged(msg_id);
    builder.tagged_logged_string(6, 1, 9999, "tagged msg");
    builder.parameter_default_i32(0x01, "TEST_PARAM", 42);

    // Also add a second valid data message to verify the parser keeps working
    let mut data_payload = Vec::new();
    data_payload.extend_from_slice(&2000u64.to_le_bytes()); // timestamp
    data_payload.extend_from_slice(&2.5f32.to_le_bytes()); // x
    builder.data(msg_id, &data_payload);

    let bytes = builder.build();
    let tmp = std::env::temp_dir().join("test_all_ignored_types.ulg");
    std::fs::write(&tmp, &bytes).expect("write temp file");

    let mut data_count = 0usize;
    let mut info_count = 0usize;
    let mut dropout_count = 0usize;
    let mut sync_count = 0usize;
    let mut remove_count = 0usize;
    let mut tagged_count = 0usize;
    let mut param_default_count = 0usize;

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(_) => data_count += 1,
            Message::LoggedMessage(_) => {}
            Message::ParameterMessage(_) => {}
            Message::InfoMessage(_) => info_count += 1,
            Message::DropoutMessage(_) => dropout_count += 1,
            Message::SyncMessage(_) => sync_count += 1,
            Message::MultiInfoMessage(_) => {}
            Message::RemoveLoggedMessage(_) => remove_count += 1,
            Message::TaggedLoggedMessage(_) => tagged_count += 1,
            Message::ParameterDefaultMessage(_) => param_default_count += 1,
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse stream with all message types without error");

    // Data messages should still work correctly even with ignored types interspersed.
    // The minimal_with_data() builder creates 1 data message, and we added 1 more above.
    assert_eq!(
        data_count, 2,
        "Both data messages should be received regardless of other message types"
    );

    // All ignored types should now be surfaced.
    assert_eq!(info_count, 1, "Info message should not be silently dropped");
    assert_eq!(dropout_count, 1, "Dropout message should not be silently dropped");
    assert_eq!(sync_count, 1, "Sync message should not be silently dropped");
    assert_eq!(remove_count, 1, "RemoveLoggedMessage should not be silently dropped");
    assert_eq!(tagged_count, 1, "TaggedLoggedString should not be silently dropped");
    assert_eq!(
        param_default_count, 1,
        "ParameterDefault should not be silently dropped"
    );

    let _ = std::fs::remove_file(&tmp);
}

/// Verify that the parser does not error on ignored message types even before implementation.
/// This tests the current behavior: ignored types should not cause parse failures.
#[test]
fn test_ignored_types_do_not_cause_errors() {
    let (mut builder, _msg_id) = ULogBuilder::minimal_with_data();
    builder.dropout(50);
    builder.sync();
    let bytes = builder.build();

    // Use consume_bytes directly to verify no error is returned.
    let mut parser = LogParser::default();
    let mut noop_data = |_: &model::DataMessage| {};
    let mut noop_log = |_: &model::LoggedStringMessage| {};
    let mut noop_param = |_: &model::ParameterMessage| {};
    parser.set_data_message_callback(&mut noop_data);
    parser.set_logged_string_message_callback(&mut noop_log);
    parser.set_parameter_message_callback(&mut noop_param);

    let result = parser.consume_bytes(&bytes);
    assert!(
        result.is_ok(),
        "Parser should not error on dropout/sync messages: {:?}",
        result.err()
    );
}
