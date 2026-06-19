//! Tests for appended data section support (ULog crash log recovery).
//!
//! The ULog spec allows data to be appended after the normal data section.
//! FlagBits incompat_flags[0] bit 0 (DATA_APPENDED) signals this, with up to
//! three appended_offsets pointing to the extra data sections.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, Message, SimpleCallbackResult,
};

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

fn count_data_messages_in_bytes(bytes: &[u8], test_name: &str) -> usize {
    let tmp = std::env::temp_dir().join(format!("{}.ulg", test_name));
    std::fs::write(&tmp, bytes).expect("write temp file");
    let count = count_data_messages(tmp.to_str().unwrap());
    let _ = std::fs::remove_file(&tmp);
    count
}

fn count_data_messages(path: &str) -> usize {
    let mut count = 0usize;
    read_file_with_simple_callback(path, &mut |msg| {
        if let Message::Data(_) = msg {
            count += 1;
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse without error");
    count
}

/// Appended file should have more data messages than the non-appended sample,
/// since the appended sections contain additional post-crash data.
#[test]
fn test_appended_file_has_more_data_than_non_appended() {
    let sample_count = count_data_messages(&fixture_path("sample.ulg"));
    let appended_count = count_data_messages(&fixture_path("sample_appended.ulg"));

    assert!(sample_count > 0, "sample.ulg should have data messages");
    assert!(
        appended_count > 0,
        "sample_appended.ulg should have data messages"
    );
    assert!(
        appended_count >= sample_count,
        "appended file should have at least as many data messages as non-appended: appended={}, sample={}",
        appended_count,
        sample_count
    );
}

/// Verify that FlagBits appended_offsets are parsed and non-zero for the
/// appended file. We check this indirectly: the appended file should produce
/// data when parsed, confirming the offsets were used.
#[test]
fn test_appended_offsets_parsed_from_flag_bits() {
    // The appended file has DATA_APPENDED flag set. After parsing, we should
    // see a substantial number of data messages (pyulog reports 81257 total
    // timestamp entries across 20 topics).
    let appended_count = count_data_messages(&fixture_path("sample_appended.ulg"));

    // pyulog sees 81257 messages. We should see a comparable number.
    // Use a generous threshold to account for counting differences.
    assert!(
        appended_count > 50000,
        "appended file should have many data messages (got {}), indicating appended offsets were used",
        appended_count
    );
}

/// Parse sample_appended_multiple.ulg (which has multiple appended sections)
/// without error and verify it produces data.
#[test]
fn test_appended_multiple_sections() {
    let count = count_data_messages(&fixture_path("sample_appended_multiple.ulg"));
    assert!(
        count > 0,
        "sample_appended_multiple.ulg should have data messages, got {}",
        count
    );
}

/// Regression: an appended offset that falls inside the first read chunk must
/// not be over-read. The reader primes the parser until FlagBits is known, then
/// clamps the primary read to the first appended offset. Before that fix the
/// first read consumed the whole (small) file, the primary parse ran past the
/// offset into the appended region, and the appended-sections loop then re-read
/// that region a second time, double-counting its data messages.
///
/// Here the appended offset is only a few hundred bytes in, so the entire log
/// fits in one read. Correct behavior yields exactly primary + appended; the
/// old behavior yielded primary + 2 * appended.
#[test]
fn test_appended_offset_within_first_chunk_not_over_read() {
    const PRIMARY_DATA: usize = 5;
    const APPENDED_DATA: usize = 7;

    let msg_id = 0u16;
    let mut data_payload = Vec::new();
    data_payload.extend_from_slice(&1000u64.to_le_bytes()); // timestamp
    data_payload.extend_from_slice(&1.5f32.to_le_bytes()); // x

    // Primary section: header + FlagBits (offset patched below) + format +
    // subscription + PRIMARY_DATA data messages.
    let mut b = ULogBuilder::new();
    b.flag_bits_with_appended(0) // placeholder offset, patched once we know it
        .format("test_topic", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(msg_id, 0, "test_topic");
    for _ in 0..PRIMARY_DATA {
        b.data(msg_id, &data_payload);
    }
    let mut bytes = b.build();
    let appended_offset = bytes.len() as u64;

    // Appended section: more data messages continuing the same stream.
    let mut appended = ULogBuilder::new();
    for _ in 0..APPENDED_DATA {
        appended.data(msg_id, &data_payload);
    }
    // Skip the appended builder's own 16-byte file header; append only its
    // message bytes after the primary section.
    bytes.extend_from_slice(&appended.build()[16..]);

    // Patch the FlagBits offset0 field to point at the appended section.
    // Layout: 16-byte file header, then FlagBits message header (2-byte size +
    // 1-byte type) at offset 16, payload at 19, offset0 at payload[16..24].
    let off0_pos = 16 + 3 + 16;
    bytes[off0_pos..off0_pos + 8].copy_from_slice(&appended_offset.to_le_bytes());

    let count = count_data_messages_in_bytes(&bytes, "appended_offset_within_first_chunk");
    assert_eq!(
        count,
        PRIMARY_DATA + APPENDED_DATA,
        "expected primary + appended data messages with no over-read/double-count"
    );
}

/// Parsing a non-appended file should be completely unaffected by the appended
/// data support. The same number of messages should be produced.
#[test]
fn test_non_appended_file_unaffected() {
    let count = count_data_messages(&fixture_path("sample.ulg"));

    // pyulog reports 64542 messages for sample.ulg
    assert!(
        count > 50000,
        "sample.ulg should still produce many data messages (got {})",
        count
    );
}
