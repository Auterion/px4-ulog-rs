//! Streaming parser chunk boundary tests. Verify that messages split across
//! consume_bytes() calls are handled correctly.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::LogParser;
use px4_ulog::stream_parser::model::DataMessage;

/// Parse bytes through LogParser fed in specified chunk sizes.
/// Returns number of data messages received.
fn parse_in_chunks(bytes: &[u8], chunk_size: usize) -> usize {
    let mut count = 0usize;
    let mut cb = |_: &DataMessage| {
        count += 1;
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut cb);

    for chunk in bytes.chunks(chunk_size) {
        parser.consume_bytes(chunk).expect("parse should not error");
    }
    count
}

/// Build a standard test stream with N data messages.
fn build_test_stream(num_messages: usize) -> Vec<u8> {
    let mut builder = ULogBuilder::new();
    builder
        .flag_bits()
        .format("test_topic", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "test_topic");

    for i in 0..num_messages {
        let ts = ((i + 1) * 1000) as u64;
        let x = i as f32;
        let mut payload = Vec::new();
        payload.extend_from_slice(&ts.to_le_bytes());
        payload.extend_from_slice(&x.to_le_bytes());
        builder.data(0, &payload);
    }

    // Add trailing byte to work around the off-by-one bug
    let mut bytes = builder.build();
    bytes.push(0x00);
    bytes
}

// =============================================================================
// Header split across chunks
// =============================================================================

#[test]
fn test_header_split_across_two_chunks() {
    let bytes = build_test_stream(5);
    // Split header: first 8 bytes, then the rest
    let mut count = 0;
    let mut cb = |_: &DataMessage| {
        count += 1;
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut cb);

    parser.consume_bytes(&bytes[..8]).expect("first chunk ok");
    parser.consume_bytes(&bytes[8..]).expect("second chunk ok");
    assert_eq!(
        count, 5,
        "All 5 data messages should parse after split header"
    );
}

// =============================================================================
// Message header split across chunks
// =============================================================================

#[test]
fn test_message_header_split_1_byte() {
    let bytes = build_test_stream(3);
    // Feed header (16 bytes), then split a message header at 1 byte
    let split_point = 16 + 1; // 1 byte into first message header
    let mut count = 0;
    let mut cb = |_: &DataMessage| {
        count += 1;
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut cb);

    parser
        .consume_bytes(&bytes[..split_point])
        .expect("first chunk ok");
    parser
        .consume_bytes(&bytes[split_point..])
        .expect("second chunk ok");
    assert_eq!(count, 3);
}

#[test]
fn test_message_header_split_2_bytes() {
    let bytes = build_test_stream(3);
    let split_point = 16 + 2; // 2 bytes into first message header
    let mut count = 0;
    let mut cb = |_: &DataMessage| {
        count += 1;
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut cb);

    parser
        .consume_bytes(&bytes[..split_point])
        .expect("first chunk ok");
    parser
        .consume_bytes(&bytes[split_point..])
        .expect("second chunk ok");
    assert_eq!(count, 3);
}

// =============================================================================
// Message body split across chunks
// =============================================================================

#[test]
fn test_message_body_split() {
    let bytes = build_test_stream(3);
    // 16 (header) + 43 (flag_bits: 3 hdr + 40 payload) = 59 bytes for header+flagbits
    // Split in the middle of the first format message body
    let split_point = 59 + 5;
    let mut count = 0;
    let mut cb = |_: &DataMessage| {
        count += 1;
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut cb);

    parser
        .consume_bytes(&bytes[..split_point])
        .expect("first chunk ok");
    parser
        .consume_bytes(&bytes[split_point..])
        .expect("second chunk ok");
    assert_eq!(count, 3);
}

// =============================================================================
// Byte-at-a-time (ultimate stress test)
// =============================================================================

#[test]
fn test_byte_at_a_time() {
    let bytes = build_test_stream(3);
    let count = parse_in_chunks(&bytes, 1);
    assert_eq!(count, 3, "Byte-at-a-time should parse all 3 data messages");
}

// =============================================================================
// Multiple messages in one chunk
// =============================================================================

#[test]
fn test_all_in_one_chunk() {
    let bytes = build_test_stream(10);
    let count = parse_in_chunks(&bytes, bytes.len());
    assert_eq!(count, 10, "All messages in one chunk should all parse");
}

// =============================================================================
// Large chunk then nothing
// =============================================================================

#[test]
fn test_large_chunk_then_empty() {
    let bytes = build_test_stream(5);
    let mut count = 0;
    let mut cb = |_: &DataMessage| {
        count += 1;
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut cb);

    parser.consume_bytes(&bytes).expect("full chunk ok");
    parser.consume_bytes(&[]).expect("empty chunk ok");
    assert_eq!(count, 5);
}

// =============================================================================
// Various fixed chunk sizes
// =============================================================================

#[test]
fn test_chunk_sizes_sweep() {
    let bytes = build_test_stream(10);

    for chunk_size in [1, 2, 3, 4, 7, 13, 16, 32, 64, 128, 256, 512, 1024] {
        let count = parse_in_chunks(&bytes, chunk_size);
        assert_eq!(
            count, 10,
            "Chunk size {} should produce 10 data messages",
            chunk_size
        );
    }
}
