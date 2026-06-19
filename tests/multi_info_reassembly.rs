//! Tests for MultiInfo ('M') message continuation/reassembly logic.
//!
//! Per the ULog spec, MultiInfo messages with is_continued=true indicate that
//! more fragments follow with the same key. The full value is the concatenation
//! of all fragments in order. These tests verify that the parser correctly
//! reassembles fragmented multi-info values.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::LogParser;
use px4_ulog::stream_parser::model::ReassembledMultiInfoMessage;

/// Helper: build a minimal ULog byte stream with flag bits, then append
/// multi-info messages and parse them, collecting reassembled results.
fn parse_multi_info_messages(build_fn: impl FnOnce(&mut ULogBuilder)) -> Vec<(String, Vec<u8>)> {
    let mut builder = ULogBuilder::new();
    builder.flag_bits();
    build_fn(&mut builder);
    let bytes = builder.build();

    let collected = std::cell::RefCell::new(Vec::new());

    let mut reassembled_cb = |msg: &ReassembledMultiInfoMessage| {
        collected
            .borrow_mut()
            .push((msg.key.clone(), msg.value.clone()));
    };

    let mut parser = LogParser::default();
    parser.set_reassembled_multi_info_callback(&mut reassembled_cb);
    parser
        .consume_bytes(&bytes)
        .expect("parse should not error on valid input");
    // Flush any remaining buffered fragments (simulates EOF)
    parser.flush_multi_info_buffer();

    collected.into_inner()
}

#[test]
fn test_single_fragment_no_continuation() {
    let results = parse_multi_info_messages(|b| {
        b.multi_info(false, "char", "sys_name", b"PX4_FMU_V5");
    });
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "sys_name");
    assert_eq!(results[0].1, b"PX4_FMU_V5");
}

#[test]
fn test_two_fragments_reassembled() {
    let results = parse_multi_info_messages(|b| {
        b.multi_info(true, "char", "hardfault_log", b"HARD")
            .multi_info(false, "char", "hardfault_log", b"FAULT");
    });
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "hardfault_log");
    assert_eq!(results[0].1, b"HARDFAULT");
}

#[test]
fn test_three_fragments_reassembled() {
    let results = parse_multi_info_messages(|b| {
        b.multi_info(true, "char", "replay", b"AAA")
            .multi_info(true, "char", "replay", b"BBB")
            .multi_info(false, "char", "replay", b"CCC");
    });
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "replay");
    assert_eq!(results[0].1, b"AAABBBCCC");
}

#[test]
fn test_different_keys_not_mixed() {
    let results = parse_multi_info_messages(|b| {
        // Interleaved fragments for two different keys
        b.multi_info(true, "char", "key_a", b"A1")
            .multi_info(true, "char", "key_b", b"B1")
            .multi_info(false, "char", "key_a", b"A2")
            .multi_info(false, "char", "key_b", b"B2");
    });
    assert_eq!(results.len(), 2);

    // Find each key's result (order depends on when is_continued=false was seen)
    let key_a = results.iter().find(|(k, _)| k == "key_a").unwrap();
    let key_b = results.iter().find(|(k, _)| k == "key_b").unwrap();

    assert_eq!(key_a.1, b"A1A2");
    assert_eq!(key_b.1, b"B1B2");
}

#[test]
fn test_single_fragment_continued_true_then_eof() {
    // is_continued=true but no more fragments arrive (file ends).
    // flush_multi_info_buffer should emit whatever we have.
    let results = parse_multi_info_messages(|b| {
        b.multi_info(true, "char", "truncated_key", b"PARTIAL");
    });
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "truncated_key");
    assert_eq!(results[0].1, b"PARTIAL");
}
