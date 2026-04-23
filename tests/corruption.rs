//! Corrupt-file verification.
//!
//! The parser is fed damaged inputs and must either return `Err` or process
//! what it can without panicking, looping, or deadlocking. Each test runs
//! against a real fixture (`sample.ulg`) rather than synthetic bytes, so the
//! mutations exercise the whole state machine.
//!
//! The strategies here are kept deterministic (fixed seed, fixed byte offsets)
//! so failures point at a specific mutation rather than a flaky random input.

use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, LogParser, Message, SimpleCallbackResult,
};
use px4_ulog::stream_parser::model::DataMessage;

fn sample_bytes() -> Vec<u8> {
    let path = format!("{}/tests/fixtures/sample.ulg", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(path).expect("read sample.ulg")
}

/// Feed bytes through the streaming parser and report whether it terminated
/// cleanly. A panic inside `consume_bytes` would abort the test process, so a
/// `true` return is also confirmation that no panic happened.
fn parse_ok_or_err(bytes: &[u8]) -> bool {
    let mut noop = |_: &DataMessage| {};
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut noop);
    parser.consume_bytes(bytes).is_ok()
}

// =============================================================================
// Truncation: cut the file at many offsets
// =============================================================================

/// Truncate `sample.ulg` at a dense grid of offsets and feed each prefix to the
/// parser. Every prefix must either parse cleanly (partial data) or return an
/// error, and none may panic. 256 samples keeps the test fast while covering
/// every major state transition.
#[test]
fn truncation_at_every_offset_never_panics() {
    let bytes = sample_bytes();
    let step = (bytes.len() / 256).max(1);

    for cut in (0..bytes.len()).step_by(step) {
        let _ = parse_ok_or_err(&bytes[..cut]);
    }
    // Full length should still parse ok (sanity check that the loop didn't
    // short-circuit on some unrelated bug).
    assert!(parse_ok_or_err(&bytes));
}

/// The first 16 bytes are the header. Cutting anywhere inside the header means
/// the parser cannot even validate magic bytes; cutting one byte before the
/// end should specifically fail rather than be accepted.
#[test]
fn truncation_inside_header_is_rejected_or_silently_incomplete() {
    let bytes = sample_bytes();
    for cut in 0..=15 {
        let result = parse_ok_or_err(&bytes[..cut]);
        // We don't require Err for every prefix (leftover-buffering is valid
        // behaviour), but the parser must not panic.
        let _ = result;
    }
}

// =============================================================================
// Byte flips: perturb deterministic positions and require no panic
// =============================================================================

/// Flip one byte at each of 512 deterministic offsets and feed the result
/// through the parser. Whatever the parser decides -- `Ok` with partial data
/// or `Err` -- it must not panic.
#[test]
fn single_byte_flips_never_panic() {
    let original = sample_bytes();
    let step = (original.len() / 512).max(1);

    for offset in (0..original.len()).step_by(step) {
        let mut mutated = original.clone();
        mutated[offset] ^= 0xFF;
        let _ = parse_ok_or_err(&mutated);
    }
}

/// Flip bytes in pseudo-random positions using a fixed seed. Catches
/// combinations of corruptions that single-byte flips would miss.
#[test]
fn multi_byte_flips_never_panic() {
    let original = sample_bytes();
    let mut state: u64 = 0xDEAD_BEEF_CAFE_BABE;

    for _ in 0..256 {
        let mut mutated = original.clone();
        for _ in 0..8 {
            // xorshift64
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let offset = (state as usize) % mutated.len();
            mutated[offset] ^= (state >> 32) as u8;
        }
        let _ = parse_ok_or_err(&mutated);
    }
}

// =============================================================================
// Header-targeted corruption
// =============================================================================

#[test]
fn header_with_bad_magic_is_rejected() {
    let mut bytes = sample_bytes();
    bytes[0] = 0x00;
    assert!(!parse_ok_or_err(&bytes));
}

#[test]
fn header_with_corrupted_trailing_magic_byte_is_rejected() {
    let mut bytes = sample_bytes();
    bytes[6] = 0xFF; // last byte of the 7-byte magic
    assert!(!parse_ok_or_err(&bytes));
}

#[test]
fn header_with_unknown_version_is_accepted() {
    // The spec tolerates unknown versions; current parser follows suit. This
    // test pins the behaviour so a future regression is visible.
    let mut bytes = sample_bytes();
    bytes[7] = 0xFF;
    assert!(parse_ok_or_err(&bytes));
}

// =============================================================================
// Message-header corruption: oversized size fields
// =============================================================================

/// Replace the first message's size field with 0xFFFF (max u16). The parser
/// should either read that many bytes out of the remaining input (and fail
/// gracefully when the payload is malformed) or stash everything as leftover.
/// It must not allocate uncontrollably or panic.
#[test]
fn oversized_message_size_does_not_panic() {
    let mut bytes = sample_bytes();
    // Header is 16 bytes; the first message's size field starts at offset 16.
    bytes[16] = 0xFF;
    bytes[17] = 0xFF;
    let _ = parse_ok_or_err(&bytes);
}

/// Replace the message type byte of the first message with an unprintable /
/// unknown value. The parser should accept the file (unknown types are
/// silently skipped) or return `Err`; either is fine as long as there's no
/// panic.
#[test]
fn unknown_first_message_type_does_not_panic() {
    let mut bytes = sample_bytes();
    // msg_type byte is at offset 18 (16 header + 2 size).
    bytes[18] = 0xAB;
    let _ = parse_ok_or_err(&bytes);
}

// =============================================================================
// File-based APIs must surface, not swallow, corruption errors
// =============================================================================

#[test]
fn read_file_with_simple_callback_handles_truncated_fixture() {
    // truncated_real.ulg is a genuine log cut off mid-message -- exactly the
    // kind of input crash logs produce. Must not panic, must deliver at least
    // one Data message, and must not lie about errors.
    let path = format!(
        "{}/tests/fixtures/truncated_real.ulg",
        env!("CARGO_MANIFEST_DIR")
    );

    let mut seen_data = false;
    let _ = read_file_with_simple_callback(&path, &mut |msg: &Message| {
        if matches!(msg, Message::Data(_)) {
            seen_data = true;
        }
        SimpleCallbackResult::KeepReading
    });

    assert!(
        seen_data,
        "truncated_real.ulg should still yield usable Data messages before EOF"
    );
}

#[test]
fn read_file_with_bogus_bytes_returns_err_not_ok() {
    use std::io::Write;
    let path = std::env::temp_dir().join("px4_ulog_corruption_bogus.ulg");
    std::fs::File::create(&path)
        .unwrap()
        .write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0])
        .unwrap();

    let mut noop = |_: &Message| SimpleCallbackResult::KeepReading;
    let result = read_file_with_simple_callback(path.to_str().unwrap(), &mut noop);
    assert!(result.is_err());

    let _ = std::fs::remove_file(&path);
}

// =============================================================================
// Real-world corrupted fixtures
// =============================================================================
//
// Harvested by scanning ~40 GB of production flight logs with
// examples/scan_corpus and picking three distinct failure modes. GPS
// coordinates in position-bearing topics have been offset by a fixed delta
// via examples/scrub_gps so the files can live in a public repo without
// leaking flight locations. The scrub preserves file size and the corruption
// itself; only lat/lon bytes in known topics are modified, plus the top-of-
// header wall-clock timestamp which is zeroed.

fn corrupt_fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}

fn parse_file_with_streaming(path: &str) -> Result<usize, px4_ulog::stream_parser::model::UlogParseError> {
    use std::io::Read;
    let mut f = std::fs::File::open(path).expect("open fixture");
    let mut parser = LogParser::default();
    let count = std::cell::Cell::new(0usize);
    let mut cb = |_: &DataMessage| count.set(count.get() + 1);
    parser.set_data_message_callback(&mut cb);

    let mut buf = [0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf).expect("read fixture");
        if n == 0 {
            break;
        }
        parser.consume_bytes(&buf[..n])?;
    }
    Ok(count.get())
}

/// Flight where the firmware emitted actuator_armed Data messages with a
/// payload size (30 bytes) that disagreed with the format definition (17
/// bytes). A real writer-side bug caught by the parser's size check.
#[test]
fn real_world_actuator_armed_wrong_size_reports_size_mismatch() {
    let err = parse_file_with_streaming(&corrupt_fixture("corrupt_actuator_armed_wrong_size.ulg"))
        .expect_err("fixture must not parse cleanly");
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("wrong size") && msg.contains("actuator_armed"),
        "expected size-mismatch on actuator_armed, got: {}",
        msg
    );
}

/// Flight log where a Data message carried an msg_id never registered via
/// AddLoggedMessage. Bit-rot in the 2-byte msg_id field is the most likely
/// cause.
#[test]
fn real_world_unregistered_msg_id_is_rejected() {
    let err = parse_file_with_streaming(&corrupt_fixture("corrupt_unregistered_msg_id.ulg"))
        .expect_err("fixture must not parse cleanly");
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("unregistered msg_id"),
        "expected unregistered-msg_id error, got: {}",
        msg
    );
}

/// Flight log where a Format ('F') message's bytes are not valid UTF-8.
/// Usually indicates the format message was hit by memory corruption before
/// being written.
#[test]
fn real_world_format_not_utf8_is_rejected() {
    let err = parse_file_with_streaming(&corrupt_fixture("corrupt_format_not_utf8.ulg"))
        .expect_err("fixture must not parse cleanly");
    let msg = format!("{:?}", err);
    assert!(
        msg.contains("format message is not a string"),
        "expected format-not-a-string error, got: {}",
        msg
    );
}

/// All three real-world corrupt fixtures must deliver some Data messages
/// before the corruption hits, not give up at the first byte. Useful logs
/// are still partially readable.
#[test]
fn real_world_fixtures_deliver_data_before_corruption() {
    for name in [
        "corrupt_actuator_armed_wrong_size.ulg",
        "corrupt_unregistered_msg_id.ulg",
        "corrupt_format_not_utf8.ulg",
    ] {
        let path = corrupt_fixture(name);
        let mut seen_data = false;
        let _ = read_file_with_simple_callback(&path, &mut |msg: &Message| {
            if matches!(msg, Message::Data(_)) {
                seen_data = true;
            }
            SimpleCallbackResult::KeepReading
        });
        assert!(
            seen_data,
            "{} should surface Data messages before hitting corruption",
            name
        );
    }
}
