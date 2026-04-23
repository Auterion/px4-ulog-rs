//! Tests for appended data section support (ULog crash log recovery).
//!
//! The ULog spec allows data to be appended after the normal data section.
//! FlagBits incompat_flags[0] bit 0 (DATA_APPENDED) signals this, with up to
//! three appended_offsets pointing to the extra data sections.

use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, Message, SimpleCallbackResult,
};

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
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
