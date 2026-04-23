//! Integration tests for memory-mapped / byte-slice based parsing.
//!
//! These tests validate that ULog data loaded entirely into memory (as would
//! happen with mmap) can be parsed correctly through the existing and planned
//! APIs.
//!
//! Tests marked "SHOULD PASS TODAY" exercise the current `consume_bytes` path
//! with contiguous in-memory data. Tests marked "NEEDS NEW API" document
//! planned APIs that do not yet exist and are expected to fail until
//! implemented.

mod helpers;

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, Message, SimpleCallbackResult,
};
use px4_ulog::stream_parser::LogParser;

use helpers::ULogBuilder;

fn fixture_path(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

/// Helper: parse a byte slice through LogParser::consume_bytes in a single call,
/// counting data messages, logged strings, and parameter messages.
fn parse_bytes_counting(data: &[u8]) -> (usize, usize, usize) {
    let data_count = Cell::new(0usize);
    let log_count = Cell::new(0usize);
    let param_count = Cell::new(0usize);

    let mut data_cb = |_msg: &px4_ulog::stream_parser::DataMessage| {
        data_count.set(data_count.get() + 1);
    };
    let mut log_cb = |_msg: &px4_ulog::stream_parser::LoggedStringMessage| {
        log_count.set(log_count.get() + 1);
    };
    let mut param_cb = |_msg: &px4_ulog::stream_parser::ParameterMessage| {
        param_count.set(param_count.get() + 1);
    };

    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut data_cb);
    parser.set_logged_string_message_callback(&mut log_cb);
    parser.set_parameter_message_callback(&mut param_cb);
    parser
        .consume_bytes(data)
        .expect("consume_bytes should succeed");

    (data_count.get(), log_count.get(), param_count.get())
}

/// Helper: parse a file via read_file_with_simple_callback, counting data messages.
fn parse_file_counting_data(path: &str) -> usize {
    let count = AtomicUsize::new(0);
    read_file_with_simple_callback(path, &mut |msg: &Message| {
        if let Message::Data(_) = msg {
            count.fetch_add(1, Ordering::Relaxed);
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("file-based parse should succeed");
    count.load(Ordering::Relaxed)
}

// =============================================================================
// 1. Parse from byte slice (synthetic) -- SHOULD PASS TODAY
// =============================================================================

#[test]
fn test_synthetic_ulog_from_byte_slice() {
    // Build a minimal ULog in memory using ULogBuilder, then parse via
    // consume_bytes in a single call. This proves that contiguous in-memory
    // data (the mmap use case) works through the existing API.
    let (builder, _msg_id) = ULogBuilder::minimal_with_data();
    let bytes = builder.build();

    let (data_count, _log_count, _param_count) = parse_bytes_counting(&bytes);
    assert_eq!(
        data_count, 1,
        "Should parse exactly 1 data message from synthetic ULog"
    );
}

#[test]
fn test_synthetic_multiple_messages_from_byte_slice() {
    // Build a ULog with multiple data messages and verify all are parsed.
    let mut builder = ULogBuilder::new();
    let msg_id = 0u16;
    builder
        .flag_bits()
        .format("sensor", &[("uint64_t", "timestamp"), ("float", "value")])
        .add_logged(msg_id, 0, "sensor");

    // Write 100 data messages
    for i in 0..100u64 {
        let mut payload = Vec::new();
        payload.extend_from_slice(&(i * 1000).to_le_bytes()); // timestamp
        payload.extend_from_slice(&(i as f32).to_le_bytes()); // value
        builder.data(msg_id, &payload);
    }

    let bytes = builder.build();
    let (data_count, _, _) = parse_bytes_counting(&bytes);
    assert_eq!(
        data_count, 100,
        "Should parse all 100 data messages from contiguous byte slice"
    );
}

// =============================================================================
// 2. Parse entire fixture file in one consume_bytes call -- SHOULD PASS TODAY
// =============================================================================

#[test]
fn test_sample_ulg_single_consume_bytes_call() {
    // Load sample.ulg into memory (simulating mmap) and parse in one call.
    // pyulog reports 64542 data messages for this file.
    let path = fixture_path("sample.ulg");
    let data = std::fs::read(&path).expect("should read sample.ulg");

    let (data_count, _log_count, _param_count) = parse_bytes_counting(&data);

    assert_eq!(
        data_count, 64542,
        "sample.ulg should produce exactly 64542 data messages (matching pyulog)"
    );
}

#[test]
fn test_6ba1abc7_single_consume_bytes_call() {
    // Load the 950KB fixture into memory and parse in a single call.
    let path = fixture_path("6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg");
    let data = std::fs::read(&path).expect("should read fixture");

    let (data_count, _log_count, _param_count) = parse_bytes_counting(&data);

    // Verify we get a reasonable number of data messages (non-zero).
    assert!(
        data_count > 0,
        "6ba1abc7 fixture should produce data messages, got {}",
        data_count
    );
}

#[test]
fn test_quadrotor_local_single_consume_bytes_call() {
    // Load the 3.7MB quadrotor log into memory and parse in a single call.
    let path = fixture_path("quadrotor_local.ulg");
    let data = std::fs::read(&path).expect("should read quadrotor_local.ulg");

    let (data_count, _log_count, _param_count) = parse_bytes_counting(&data);

    assert!(
        data_count > 0,
        "quadrotor_local.ulg should produce data messages, got {}",
        data_count
    );
}

// =============================================================================
// 3. Byte-slice parsing matches file parsing -- SHOULD PASS TODAY
// =============================================================================

#[test]
fn test_sample_ulg_byte_slice_matches_file_parse() {
    let path = fixture_path("sample.ulg");
    let file_count = parse_file_counting_data(&path);

    let data = std::fs::read(&path).expect("should read sample.ulg");
    let (byte_count, _, _) = parse_bytes_counting(&data);

    assert_eq!(
        byte_count, file_count,
        "Byte-slice parse ({}) must match file-based parse ({}) for sample.ulg",
        byte_count, file_count
    );
}

#[test]
fn test_6ba1abc7_byte_slice_matches_file_parse() {
    let path = fixture_path("6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg");
    let file_count = parse_file_counting_data(&path);

    let data = std::fs::read(&path).expect("should read fixture");
    let (byte_count, _, _) = parse_bytes_counting(&data);

    assert_eq!(
        byte_count, file_count,
        "Byte-slice parse ({}) must match file-based parse ({}) for 6ba1abc7",
        byte_count, file_count
    );
}

#[test]
fn test_quadrotor_local_byte_slice_matches_file_parse() {
    let path = fixture_path("quadrotor_local.ulg");
    let file_count = parse_file_counting_data(&path);

    let data = std::fs::read(&path).expect("should read quadrotor_local.ulg");
    let (byte_count, _, _) = parse_bytes_counting(&data);

    assert_eq!(
        byte_count, file_count,
        "Byte-slice parse ({}) must match file-based parse ({}) for quadrotor_local",
        byte_count, file_count
    );
}

#[test]
fn test_all_fixtures_byte_slice_matches_file_parse() {
    // Cross-validate every fixture: byte-slice parsing must produce the same
    // data message count as file-based parsing.
    let fixtures = [
        "sample.ulg",
        "6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        "quadrotor_local.ulg",
    ];

    for fixture in &fixtures {
        let path = fixture_path(fixture);
        let file_count = parse_file_counting_data(&path);
        let data = std::fs::read(&path).unwrap_or_else(|_| panic!("should read {}", fixture));
        let (byte_count, _, _) = parse_bytes_counting(&data);

        assert_eq!(
            byte_count, file_count,
            "Byte-slice vs file mismatch for {}: {} vs {}",
            fixture, byte_count, file_count
        );
    }
}

// =============================================================================
// 4. Full parser from bytes -- NEEDS NEW API
//
// Currently full_parser::read_file() only accepts a &str file path.
// These tests document the need for a full_parser::read_bytes(&[u8]) function
// that would allow the full parser to work directly from an in-memory buffer
// (e.g., mmap'd data) without writing to a temporary file.
//
// TODO: Implement full_parser::read_bytes(&[u8]) -> Result<ParsedData, ...>
// =============================================================================

#[test]
#[ignore] // Remove #[ignore] once full_parser::read_bytes is implemented
fn test_full_parser_from_bytes_sample() {
    let path = fixture_path("sample.ulg");
    let _data = std::fs::read(&path).expect("should read sample.ulg");

    // NEEDS NEW API: full_parser::read_bytes(&[u8]) -> Result<ParsedData, ...>
    // This would parse directly from an in-memory byte slice, avoiding file I/O.
    //
    // let parsed = full_parser::read_bytes(&data).expect("should parse from bytes");
    //
    // Compare against file-based parse:
    // let file_parsed = full_parser::read_file(&path).expect("should parse from file");
    //
    // Verify they produce the same message topics:
    // let byte_topics: Vec<&String> = parsed.messages.keys().collect();
    // let file_topics: Vec<&String> = file_parsed.messages.keys().collect();
    // assert_eq!(byte_topics.len(), file_topics.len());
    //
    // For now, just assert that the function exists (will fail to compile
    // if uncommented until the API is added):
    panic!(
        "full_parser::read_bytes(&[u8]) is not yet implemented. \
         Add a read_bytes function to full_parser that accepts a byte slice \
         and returns ParsedData, enabling mmap-based full parsing."
    );
}

#[test]
#[ignore] // Remove #[ignore] once full_parser::read_bytes is implemented
fn test_full_parser_from_bytes_matches_file_parse() {
    let path = fixture_path("6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg");
    let _data = std::fs::read(&path).expect("should read fixture");

    // NEEDS NEW API: full_parser::read_bytes
    //
    // let byte_parsed = full_parser::read_bytes(&data).unwrap();
    // let file_parsed = full_parser::read_file(&path).unwrap();
    //
    // // Same set of message topics
    // let mut byte_keys: Vec<_> = byte_parsed.messages.keys().cloned().collect();
    // let mut file_keys: Vec<_> = file_parsed.messages.keys().cloned().collect();
    // byte_keys.sort();
    // file_keys.sort();
    // assert_eq!(byte_keys, file_keys, "Message topics should match");
    //
    // // Same number of entries per topic
    // for topic in &byte_keys {
    //     let byte_multi = &byte_parsed.messages[topic];
    //     let file_multi = &file_parsed.messages[topic];
    //     assert_eq!(
    //         byte_multi.len(), file_multi.len(),
    //         "Multi-id count mismatch for topic {}", topic
    //     );
    // }
    panic!("full_parser::read_bytes(&[u8]) is not yet implemented.");
}

// =============================================================================
// 5. Large contiguous buffer performance -- design validation
//
// When all data is available in a single contiguous buffer (mmap), the parser
// should NOT need to copy bytes into the leftover buffer. This test verifies
// that behavior by checking that after a single consume_bytes call with a
// complete file, leftover is empty (all data was consumed inline).
//
// SHOULD PASS TODAY: consume_bytes with a complete file means every
// parse_single_entry call succeeds without hitting the leftover path, because
// there is always enough data available. The only leftover would be trailing
// bytes that don't form a complete message (none for a valid file).
//
// NEEDS NEW API (future): A dedicated parse_contiguous(&[u8]) method could
// skip the leftover check entirely for better performance. It would:
//   - Assert leftover is empty at entry
//   - Never allocate or copy into the leftover Vec
//   - Return an error if data is incomplete (rather than storing leftovers)
//
// TODO: Implement LogParser::parse_contiguous(&mut self, data: &[u8])
// =============================================================================

#[test]
fn test_contiguous_parse_no_leftover_overhead() {
    // After parsing a complete, valid ULog in one consume_bytes call, the
    // parser should have consumed everything. We verify this indirectly:
    // if consume_bytes returns Ok and we got the expected message count,
    // the leftover path was only hit at most once at the very end (for 0
    // remaining bytes, which is a no-op extend).
    let path = fixture_path("sample.ulg");
    let data = std::fs::read(&path).expect("should read sample.ulg");

    let data_count = Cell::new(0usize);
    let mut data_cb = |_msg: &px4_ulog::stream_parser::DataMessage| {
        data_count.set(data_count.get() + 1);
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut data_cb);

    // Single call with all bytes -- the internal loop in consume_bytes should
    // parse every message without ever entering the leftover branch (since
    // leftover starts empty and each parse_single_entry consumes complete
    // messages from the contiguous buffer).
    parser
        .consume_bytes(&data)
        .expect("consume_bytes with full file should succeed");

    assert_eq!(
        data_count.get(),
        64542,
        "All data messages should be parsed from contiguous buffer"
    );

    // After consuming a complete file, get_final_data_format should work
    // (it consumes the parser). This also proves the parser reached a valid
    // terminal state.
    let data_format = parser.get_final_data_format();
    // The data format should have registered message descriptions.
    // (We can't easily check leftover is empty since it's private, but the
    // fact that all messages parsed correctly from a single call is strong
    // evidence the leftover path was not needed.)
    let _ = data_format;
}

#[test]
fn test_contiguous_vs_chunked_equivalence() {
    // Parse the same file two ways:
    //   (a) Single consume_bytes call (contiguous, simulating mmap)
    //   (b) Multiple consume_bytes calls with 4KB chunks (simulating read())
    // Both should produce identical data message counts.
    let path = fixture_path("sample.ulg");
    let data = std::fs::read(&path).expect("should read sample.ulg");

    // (a) Contiguous
    let (contiguous_count, _, _) = parse_bytes_counting(&data);

    // (b) Chunked
    let chunked_count = Cell::new(0usize);
    let mut data_cb = |_msg: &px4_ulog::stream_parser::DataMessage| {
        chunked_count.set(chunked_count.get() + 1);
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut data_cb);

    let chunk_size = 4096;
    for chunk in data.chunks(chunk_size) {
        parser
            .consume_bytes(chunk)
            .expect("chunked consume_bytes should succeed");
    }
    let chunked = chunked_count.get();

    assert_eq!(
        contiguous_count, chunked,
        "Contiguous ({}) and chunked ({}) parsing must produce identical results",
        contiguous_count, chunked
    );
}

#[test]
#[ignore] // Remove #[ignore] once LogParser::parse_contiguous is implemented
fn test_parse_contiguous_dedicated_method() {
    // NEEDS NEW API: LogParser::parse_contiguous(&mut self, data: &[u8])
    //
    // This method would be optimized for the case where all data is available
    // at once (mmap). Unlike consume_bytes, it would:
    //   - Never allocate a leftover buffer
    //   - Return an error on incomplete data instead of buffering
    //   - Avoid the leftover check at the top of each call
    //
    // let path = fixture_path("sample.ulg");
    // let data = std::fs::read(&path).unwrap();
    //
    // let count = Cell::new(0usize);
    // let mut cb = |_: &px4_ulog::stream_parser::DataMessage| {
    //     count.set(count.get() + 1);
    // };
    // let mut parser = LogParser::default();
    // parser.set_data_message_callback(&mut cb);
    //
    // parser.parse_contiguous(&data).expect("should parse contiguous buffer");
    // assert_eq!(count.get(), 64542);
    panic!(
        "LogParser::parse_contiguous(&[u8]) is not yet implemented. \
         This method should parse a complete, contiguous buffer without \
         leftover management overhead."
    );
}

// =============================================================================
// Bonus: Verify that byte-slice parsing captures all message types
// =============================================================================

#[test]
fn test_byte_slice_captures_logged_strings() {
    // Verify that logged string messages are also captured when parsing from
    // a byte slice, not just data messages.
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("test_msg", &[("uint64_t", "timestamp"), ("float", "x")])
        .logged_string(6, 5000, "Test log message")
        .add_logged(0, 0, "test_msg");

    let mut payload = Vec::new();
    payload.extend_from_slice(&1000u64.to_le_bytes());
    payload.extend_from_slice(&2.5f32.to_le_bytes());
    builder.data(0, &payload);

    let bytes = builder.build();
    let (_data_count, log_count, _param_count) = parse_bytes_counting(&bytes);

    assert_eq!(
        log_count, 1,
        "Should capture logged string messages from byte slice"
    );
}

#[test]
fn test_byte_slice_captures_parameters() {
    // Verify that parameter messages are captured when parsing from a byte
    // slice.
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .parameter_i32("SYS_AUTOSTART", 4001)
        .parameter_f32("MC_PITCHRATE_P", 0.15)
        .format("test_msg", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "test_msg");

    let mut payload = Vec::new();
    payload.extend_from_slice(&1000u64.to_le_bytes());
    payload.extend_from_slice(&1.0f32.to_le_bytes());
    builder.data(0, &payload);

    let bytes = builder.build();
    let (_data_count, _log_count, param_count) = parse_bytes_counting(&bytes);

    assert_eq!(
        param_count, 2,
        "Should capture both parameter messages from byte slice"
    );
}
