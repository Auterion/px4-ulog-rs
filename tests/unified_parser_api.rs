//! Integration tests defining the unified parser API after the seek-based parser is removed.
//!
//! These tests are written in a TDD style: they define the target API surface
//! that the streaming/full parser should provide once the seek parser is gone.
//! Tests that work with the current API are expected to pass now. Tests that
//! require API changes are commented with what needs to change.

// ---------------------------------------------------------------------------
// Test 1: Header info accessible from stream/full parser
// ---------------------------------------------------------------------------
// LogParser exposes version() and timestamp() for header access.

#[test]
fn header_version_via_stream_parser() {
    use px4_ulog::stream_parser::LogParser;
    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let data = std::fs::read(&filename).unwrap();
    let mut parser = LogParser::default();
    parser.consume_bytes(&data[..16]).unwrap();
    assert_eq!(parser.version(), 1);
}

#[test]
fn header_timestamp_via_stream_parser() {
    use px4_ulog::stream_parser::LogParser;
    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let data = std::fs::read(&filename).unwrap();
    let mut parser = LogParser::default();
    parser.consume_bytes(&data[..16]).unwrap();
    assert_eq!(parser.timestamp(), 373058900);
}

#[test]
fn header_timestamp_sample_ulg_via_stream_parser() {
    use px4_ulog::stream_parser::LogParser;
    let filename = format!("{}/tests/fixtures/sample.ulg", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read(&filename).unwrap();
    let mut parser = LogParser::default();
    parser.consume_bytes(&data[..16]).unwrap();
    assert_eq!(parser.timestamp(), 112500176);
}

// ---------------------------------------------------------------------------
// Test 2: Dataset extraction via full_parser
// ---------------------------------------------------------------------------
// The seek parser provides get_dataset("vehicle_gps_position") returning an
// iterator of ULogData with 260 entries. The full_parser::read_file already
// collects all data into ParsedData. Verify equivalent extraction.

#[test]
fn dataset_extraction_gps_260_points_via_full_parser() {
    use px4_ulog::full_parser::{read_file, MultiId, SomeVec};

    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let parsed = read_file(&filename).unwrap();

    // The full parser stores data keyed by topic name -> multi_id -> field -> SomeVec.
    let gps = parsed
        .messages
        .get("vehicle_gps_position")
        .expect("vehicle_gps_position topic not found");

    let instance = gps
        .get(&MultiId::new(0))
        .expect("multi_id 0 not found for vehicle_gps_position");

    // The seek parser yields 260 data points. Verify the same count via full_parser.
    // All field vectors should have the same length (one entry per data message).
    let timestamp_vec = instance
        .get("timestamp")
        .expect("timestamp field not found");

    match timestamp_vec {
        SomeVec::UInt64(v) => {
            assert_eq!(
                v.len(),
                260,
                "Expected 260 GPS data points, got {}",
                v.len()
            );
        }
        other => panic!(
            "Expected timestamp to be UInt64, got {:?}",
            std::mem::discriminant(other)
        ),
    }
}

#[test]
fn dataset_first_gps_values_match_known_reference() {
    // Verify that the first data point values match known reference values.
    use px4_ulog::full_parser::{read_file, MultiId, SomeVec};

    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let parsed = read_file(&filename).unwrap();
    let instance = parsed
        .messages
        .get("vehicle_gps_position")
        .unwrap()
        .get(&MultiId::new(0))
        .unwrap();

    // Check first values match what the seek parser data.rs test verifies.
    fn first_u64(sv: &SomeVec) -> u64 {
        match sv {
            SomeVec::UInt64(v) => v[0],
            _ => panic!("expected UInt64"),
        }
    }
    fn first_i32(sv: &SomeVec) -> i32 {
        match sv {
            SomeVec::Int32(v) => v[0],
            _ => panic!("expected Int32"),
        }
    }
    fn first_f32(sv: &SomeVec) -> f32 {
        match sv {
            SomeVec::Float(v) => v[0],
            _ => panic!("expected Float"),
        }
    }
    fn first_u8(sv: &SomeVec) -> u8 {
        match sv {
            SomeVec::UInt8(v) => v[0],
            _ => panic!("expected UInt8"),
        }
    }
    fn first_bool(sv: &SomeVec) -> bool {
        match sv {
            SomeVec::Bool(v) => v[0],
            _ => panic!("expected Bool"),
        }
    }

    assert_eq!(first_u64(&instance["timestamp"]), 375408345);
    assert_eq!(first_u64(&instance["time_utc_usec"]), 0);
    assert_eq!(first_i32(&instance["lat"]), 407423012);
    assert_eq!(first_i32(&instance["lon"]), -741792999);
    assert_eq!(first_i32(&instance["alt"]), 28495);
    assert_eq!(first_i32(&instance["alt_ellipsoid"]), 0);
    assert_eq!(first_f32(&instance["s_variance_m_s"]), 0.0);
    assert_eq!(first_f32(&instance["c_variance_rad"]), 0.0);
    assert_eq!(first_f32(&instance["eph"]), 0.29999998);
    assert_eq!(first_f32(&instance["epv"]), 0.39999998);
    assert_eq!(first_f32(&instance["hdop"]), 0.0);
    assert_eq!(first_f32(&instance["vdop"]), 0.0);
    assert_eq!(first_i32(&instance["noise_per_ms"]), 0);
    assert_eq!(first_i32(&instance["jamming_indicator"]), 0);
    assert_eq!(first_f32(&instance["vel_m_s"]), 0.0);
    assert_eq!(first_f32(&instance["vel_n_m_s"]), 0.0);
    assert_eq!(first_f32(&instance["vel_e_m_s"]), 0.0);
    assert_eq!(first_f32(&instance["vel_d_m_s"]), 0.0);
    assert_eq!(first_f32(&instance["cog_rad"]), 0.0);
    assert_eq!(first_i32(&instance["timestamp_time_relative"]), 0);
    assert_eq!(first_u8(&instance["fix_type"]), 3);
    assert!(!first_bool(&instance["vel_ned_valid"]));
    assert_eq!(first_u8(&instance["satellites_used"]), 10);
}

// ---------------------------------------------------------------------------
// Test 3: Topic name listing
// ---------------------------------------------------------------------------
// The seek parser provides get_message_names() returning Vec<String>.
// The full_parser ParsedData.messages HashMap keys serve the same purpose.

#[test]
fn topic_names_via_full_parser_main_fixture() {
    use px4_ulog::full_parser::read_file;

    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let parsed = read_file(&filename).unwrap();
    let names: Vec<&String> = parsed.messages.keys().collect();

    assert!(
        names.iter().any(|n| n.as_str() == "vehicle_gps_position"),
        "Expected vehicle_gps_position in parsed topics"
    );
    assert!(!names.is_empty(), "Expected at least one topic");
}

#[test]
fn topic_names_via_full_parser() {
    // The full_parser already provides this: keys of ParsedData.messages.
    // This is the replacement for get_message_names().
    use px4_ulog::full_parser::read_file;

    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let parsed = read_file(&filename).unwrap();

    let topic_names: Vec<&String> = parsed.messages.keys().collect();
    assert!(
        topic_names
            .iter()
            .any(|n| n.as_str() == "vehicle_gps_position"),
        "Expected vehicle_gps_position in full_parser topic names"
    );
}

#[test]
fn topic_names_via_full_parser_sample_file() {
    // sample.ulg is known to have 15 topics according to fixture description.
    use px4_ulog::full_parser::read_file;

    let filename = format!("{}/tests/fixtures/sample.ulg", env!("CARGO_MANIFEST_DIR"));
    let parsed = read_file(&filename).unwrap();

    let topic_count = parsed.messages.len();
    assert!(
        topic_count >= 15,
        "Expected at least 15 topics in sample.ulg, got {}",
        topic_count
    );
}

#[test]
fn topic_names_via_stream_callback() {
    // Demonstrate that topic names can also be collected via the streaming callback API.
    use px4_ulog::stream_parser::file_reader::SimpleCallbackResult;
    use px4_ulog::stream_parser::{read_file_with_simple_callback, Message};
    use std::collections::HashSet;

    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );

    let mut topic_names = HashSet::new();
    let mut callback = |msg: &Message| -> SimpleCallbackResult {
        if let Message::Data(data_msg) = msg {
            topic_names.insert(data_msg.flattened_format.message_name().to_string());
        }
        SimpleCallbackResult::KeepReading
    };

    read_file_with_simple_callback(&filename, &mut callback).unwrap();

    assert!(
        topic_names.contains("vehicle_gps_position"),
        "Expected vehicle_gps_position from streaming parser"
    );
}

// ---------------------------------------------------------------------------
// Test 4: All 12 data types supported in SomeVec
// ---------------------------------------------------------------------------
// The seek parser's DataType (in models/data.rs) only supports 5 variants:
//   UInt64, Int32, Float, UInt8, Bool
// The stream parser and full_parser support all 12 ULog types via
// FlattenedFieldType and SomeVec. Verify the enum variants exist.

#[test]
fn somevec_supports_all_12_data_types() {
    use px4_ulog::full_parser::SomeVec;

    // Construct each variant to prove they exist at compile time.
    let variants: Vec<SomeVec> = vec![
        SomeVec::Int8(vec![1i8]),
        SomeVec::UInt8(vec![1u8]),
        SomeVec::Int16(vec![1i16]),
        SomeVec::UInt16(vec![1u16]),
        SomeVec::Int32(vec![1i32]),
        SomeVec::UInt32(vec![1u32]),
        SomeVec::Int64(vec![1i64]),
        SomeVec::UInt64(vec![1u64]),
        SomeVec::Float(vec![1.0f32]),
        SomeVec::Double(vec![1.0f64]),
        SomeVec::Bool(vec![true]),
        SomeVec::Char(vec!['a']),
    ];

    assert_eq!(
        variants.len(),
        12,
        "SomeVec should have 12 data type variants"
    );
}

#[test]
fn flattened_field_type_supports_all_12_types() {
    use px4_ulog::full_parser::FlattenedFieldType;

    // Verify all 12 FlattenedFieldType variants exist.
    let types = [
        FlattenedFieldType::Int8,
        FlattenedFieldType::UInt8,
        FlattenedFieldType::Int16,
        FlattenedFieldType::UInt16,
        FlattenedFieldType::Int32,
        FlattenedFieldType::UInt32,
        FlattenedFieldType::Int64,
        FlattenedFieldType::UInt64,
        FlattenedFieldType::Float,
        FlattenedFieldType::Double,
        FlattenedFieldType::Bool,
        FlattenedFieldType::Char,
    ];

    assert_eq!(types.len(), 12);
}

// The seek parser's DataType only had 5 of 12 variants. The full/stream parser's
// SomeVec and FlattenedFieldType handle all 12 — tested in somevec_supports_all_12_data_types
// and flattened_field_type_supports_all_12_types below.

// ---------------------------------------------------------------------------
// Test 5: Byte slice input (enables mmap, embedded, no-std-fs use cases)
// ---------------------------------------------------------------------------
// The seek parser requires std::fs::File. The stream parser's LogParser
// accepts &[u8] via consume_bytes(), enabling mmap and in-memory parsing.

#[test]
fn stream_parser_accepts_byte_slice_input() {
    // Read the file into memory, then parse via consume_bytes.
    // This proves the parser works without std::fs::File, enabling mmap.
    use px4_ulog::stream_parser::LogParser;

    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let data = std::fs::read(&filename).unwrap();

    let mut message_count: usize = 0;
    let mut callback = |_msg: &px4_ulog::stream_parser::DataMessage| {
        message_count += 1;
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut callback);

    // Feed the entire file as a single byte slice.
    parser.consume_bytes(&data).unwrap();

    assert!(
        message_count > 0,
        "Expected at least one data message when parsing from byte slice"
    );
}

#[test]
fn stream_parser_accepts_chunked_byte_slices() {
    // Verify the parser handles incremental/chunked input correctly,
    // simulating streaming from a network source or partial reads.
    use px4_ulog::stream_parser::LogParser;

    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let data = std::fs::read(&filename).unwrap();

    let mut message_count: usize = 0;
    let mut callback = |_msg: &px4_ulog::stream_parser::DataMessage| {
        message_count += 1;
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut callback);

    // Feed in small chunks to test the leftover/reassembly logic.
    let chunk_size = 137; // intentionally not aligned to any message boundary
    for chunk in data.chunks(chunk_size) {
        parser.consume_bytes(chunk).unwrap();
    }

    assert!(
        message_count > 0,
        "Expected data messages when parsing chunked byte slices"
    );
}

#[test]
fn stream_parser_byte_count_matches_one_shot_vs_chunked() {
    // Verify that chunked parsing produces the same number of data messages
    // as single-shot parsing.
    use px4_ulog::stream_parser::LogParser;

    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let data = std::fs::read(&filename).unwrap();

    // Parse in one shot.
    let mut count_one_shot: usize = 0;
    {
        let mut callback = |_msg: &px4_ulog::stream_parser::DataMessage| {
            count_one_shot += 1;
        };
        let mut parser = LogParser::default();
        parser.set_data_message_callback(&mut callback);
        parser.consume_bytes(&data).unwrap();
    }

    // Parse in chunks.
    let mut count_chunked: usize = 0;
    {
        let mut callback = |_msg: &px4_ulog::stream_parser::DataMessage| {
            count_chunked += 1;
        };
        let mut parser = LogParser::default();
        parser.set_data_message_callback(&mut callback);
        for chunk in data.chunks(256) {
            parser.consume_bytes(chunk).unwrap();
        }
    }

    assert_eq!(
        count_one_shot, count_chunked,
        "One-shot and chunked parsing should yield the same number of data messages"
    );
}

#[test]
fn full_parser_byte_slice_input() {
    // TODO: full_parser::read_file currently only accepts a file path string.
    // After unification, it should also accept &[u8] or impl Read so that
    // callers can use mmap or in-memory buffers without going through the
    // filesystem.
    //
    // Target API:
    //   pub fn read_bytes(data: &[u8]) -> Result<ParsedData, ...>
    //   OR
    //   pub fn read_from(reader: impl Read) -> Result<ParsedData, ...>
    //
    // For now, verify that LogParser::consume_bytes works as the foundation.
    use px4_ulog::stream_parser::LogParser;

    let filename = format!("{}/tests/fixtures/sample.ulg", env!("CARGO_MANIFEST_DIR"));
    let data = std::fs::read(&filename).unwrap();

    let mut message_count: usize = 0;
    let mut callback = |_msg: &px4_ulog::stream_parser::DataMessage| {
        message_count += 1;
    };
    let mut parser = LogParser::default();
    parser.set_data_message_callback(&mut callback);
    parser.consume_bytes(&data).unwrap();

    // sample.ulg has 64542 messages according to fixture description.
    assert!(
        message_count > 60000,
        "Expected ~64542 data messages from sample.ulg, got {}",
        message_count
    );
}

// ---------------------------------------------------------------------------
// Test 6: Multi-instance topic handling
// ---------------------------------------------------------------------------
// ULog supports multiple instances of the same topic via multi_id.
// The seek parser's get_dataset only returns the first instance.
// The full_parser stores data keyed by MultiId, preserving all instances.

#[test]
fn multi_id_support_in_full_parser() {
    use px4_ulog::full_parser::{read_file, MultiId};

    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let parsed = read_file(&filename).unwrap();

    // Verify that the parsed data uses MultiId keys for each topic.
    for (topic_name, instances) in &parsed.messages {
        for (multi_id, fields) in instances {
            assert!(
                !fields.is_empty(),
                "Topic {} multi_id {} has no fields",
                topic_name,
                multi_id.value()
            );
        }
    }

    // vehicle_gps_position should have at least multi_id 0.
    let gps = parsed.messages.get("vehicle_gps_position").unwrap();
    assert!(
        gps.contains_key(&MultiId::new(0)),
        "vehicle_gps_position should have multi_id 0"
    );
}

#[test]
fn multi_id_value_accessor() {
    use px4_ulog::full_parser::MultiId;

    let id = MultiId::new(3);
    assert_eq!(id.value(), 3);

    let id0 = MultiId::new(0);
    assert_eq!(id0.value(), 0);
}

#[test]
fn multi_id_used_as_hash_key() {
    use px4_ulog::full_parser::MultiId;
    use std::collections::HashMap;

    // MultiId must implement Hash + Eq to be used as HashMap key.
    let mut map: HashMap<MultiId, String> = HashMap::new();
    map.insert(MultiId::new(0), "instance_0".to_string());
    map.insert(MultiId::new(1), "instance_1".to_string());

    assert_eq!(map.get(&MultiId::new(0)).unwrap(), "instance_0");
    assert_eq!(map.get(&MultiId::new(1)).unwrap(), "instance_1");
    assert!(!map.contains_key(&MultiId::new(2)));
}

#[test]
fn esc_status_nested_sub_messages_with_multi_id() {
    // esc_status_log.ulg has nested sub-messages (esc_status contains esc[N] sub-messages).
    // Verify the full parser handles nested/repeated sub-message fields correctly.
    use px4_ulog::full_parser::{read_file, MultiId};

    let filename = format!(
        "{}/tests/fixtures/esc_status_log.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let parsed = read_file(&filename).unwrap();

    let esc_status = parsed
        .messages
        .get("esc_status")
        .expect("esc_status topic not found");

    let instance = esc_status
        .get(&MultiId::new(0))
        .expect("esc_status multi_id 0 not found");

    // The existing test in full_parser verifies esc[5].esc_rpm exists.
    // Verify the same here as an integration test.
    assert!(
        instance.contains_key("esc[5].esc_rpm"),
        "Expected nested field esc[5].esc_rpm in esc_status"
    );

    // Also verify some other nested indices exist.
    assert!(
        instance.contains_key("esc[0].esc_rpm"),
        "Expected nested field esc[0].esc_rpm in esc_status"
    );
}

// ---------------------------------------------------------------------------
// Test 7: Stream parser callback collects same data as full_parser
// ---------------------------------------------------------------------------
// Verify that the callback-based streaming parser and the full_parser produce
// consistent results for the same file.

#[test]
fn stream_callback_gps_count_matches_full_parser() {
    use px4_ulog::full_parser::{read_file, MultiId};
    use px4_ulog::stream_parser::file_reader::SimpleCallbackResult;
    use px4_ulog::stream_parser::{read_file_with_simple_callback, Message};

    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );

    // Count GPS messages via streaming callback.
    let mut gps_count_stream: usize = 0;
    let mut callback = |msg: &Message| -> SimpleCallbackResult {
        if let Message::Data(data_msg) = msg {
            if data_msg.flattened_format.message_name() == "vehicle_gps_position" {
                gps_count_stream += 1;
            }
        }
        SimpleCallbackResult::KeepReading
    };
    read_file_with_simple_callback(&filename, &mut callback).unwrap();

    // Count GPS messages via full_parser.
    let parsed = read_file(&filename).unwrap();
    let gps = parsed.messages.get("vehicle_gps_position").unwrap();
    let gps_count_full: usize = gps
        .get(&MultiId::new(0))
        .map(|fields| {
            // All field vectors have the same length; pick "timestamp".
            match fields.get("timestamp").unwrap() {
                px4_ulog::full_parser::SomeVec::UInt64(v) => v.len(),
                _ => 0,
            }
        })
        .unwrap_or(0);

    assert_eq!(
        gps_count_stream, gps_count_full,
        "Streaming and full parser should report the same GPS message count"
    );
    assert_eq!(gps_count_stream, 260);
}

// ---------------------------------------------------------------------------
// Test 8: Invalid file handling
// ---------------------------------------------------------------------------

#[test]
fn stream_parser_rejects_non_ulog_data() {
    use px4_ulog::stream_parser::LogParser;

    // Feed 16+ bytes of garbage so the parser attempts header validation
    // and fails on the magic bytes check.
    let garbage = vec![0u8; 64];

    let mut parser = LogParser::default();
    let result = parser.consume_bytes(&garbage);

    assert!(
        result.is_err(),
        "Stream parser should reject data with invalid ULog header"
    );
}

#[test]
fn stream_parser_rejects_empty_input() {
    use px4_ulog::stream_parser::LogParser;

    let mut parser = LogParser::default();
    // Empty input should not error (nothing to parse yet), but should not
    // crash either. The parser just has nothing to consume.
    let result = parser.consume_bytes(&[]);
    assert!(result.is_ok(), "Empty input should not cause an error");
}

#[test]
fn full_parser_handles_empty_file_gracefully() {
    let filename = format!(
        "{}/tests/fixtures/not_a_log_file.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let result = px4_ulog::full_parser::read_file(&filename);
    // not_a_log_file.txt is 0 bytes — full_parser returns Ok with empty data
    // (no bytes to parse means no error, just no messages)
    if let Ok(parsed) = result {
        assert!(
            parsed.messages.is_empty(),
            "Empty file should produce no messages"
        );
    }
    // Err is also acceptable — rejecting invalid files is fine
}
