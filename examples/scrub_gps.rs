// Apply a lat/lon offset to every position-bearing Data message in a ULog
// file. Used to sanitise real flight logs before committing them as test
// fixtures: trajectory shape survives, the absolute location does not.
//
// The offset is REQUIRED at runtime and is not committed anywhere. Pick
// something, use it to scrub, and throw the number away. Anyone reading the
// fixture later has no way to recover the original coordinates. If the
// offset lived in the source, reversing the scrub would be trivial.
//
// Two passes:
//   1. Parse the log with the streaming parser to learn which msg_ids are
//      bound to position-bearing topics and where lat/lon live inside each
//      record.
//   2. Walk the raw bytes, copy everything verbatim to output, but for Data
//      messages matching a position topic overwrite lat/lon bytes with the
//      offset-adjusted values. The top-of-header wall-clock timestamp is
//      zeroed.
//
// Deliberately NON-destructive on parse errors: we process messages up to
// the first error, then stream the remaining bytes unchanged. That keeps
// the corruption intact at the same byte offset.
//
// Usage:
//   cargo run --release --example scrub_gps -- <in.ulg> <out.ulg> <dlat_deg> <dlon_deg>

use px4_ulog::stream_parser::file_reader::LogParser;
use px4_ulog::stream_parser::model::{DataMessage, FlattenedFieldType};
use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};

/// Topics and field names that carry lat/lon we want to scrub.
/// Field types are determined at runtime from the FlattenedFormat.
const POSITION_FIELDS: &[(&str, &[&str])] = &[
    ("vehicle_gps_position", &["lat", "lon"]),
    ("vehicle_global_position", &["lat", "lon"]),
    (
        "sensor_gps",
        &["lat", "lon", "latitude_deg", "longitude_deg"],
    ),
    ("home_position", &["lat", "lon"]),
    ("vehicle_local_position", &["ref_lat", "ref_lon"]),
    (
        "position_setpoint_triplet",
        &[
            "current.lat",
            "current.lon",
            "previous.lat",
            "previous.lon",
            "next.lat",
            "next.lon",
        ],
    ),
    ("estimator_global_position", &["lat", "lon"]),
    ("estimator_gps_status", &["lat", "lon"]),
];

#[derive(Clone, Debug)]
struct FieldLoc {
    name: String,
    field_type: FlattenedFieldType,
    offset: u16, // byte offset within a Data message payload (after the 2-byte msg_id)
    is_lat: bool,
}

type ScrubMap = HashMap<u16, Vec<FieldLoc>>;

fn collect_position_layout(path: &str) -> std::io::Result<ScrubMap> {
    let mut f = File::open(path)?;
    let mut parser = LogParser::default();
    let scrub: std::cell::RefCell<ScrubMap> = std::cell::RefCell::new(HashMap::new());
    let seen: std::cell::RefCell<HashMap<u16, String>> = std::cell::RefCell::new(HashMap::new());

    let mut cb = |msg: &DataMessage| {
        let msg_id = msg.msg_id;
        let mut seen = seen.borrow_mut();
        if seen.contains_key(&msg_id) {
            return;
        }
        let topic = msg.flattened_format.message_name.clone();
        seen.insert(msg_id, topic.clone());

        let Some((_, field_names)) = POSITION_FIELDS.iter().find(|(t, _)| *t == topic) else {
            return;
        };

        let mut locs = Vec::new();
        for field in &msg.flattened_format.fields {
            let last = field
                .flattened_field_name
                .rsplit('.')
                .next()
                .unwrap_or(&field.flattened_field_name);
            if field_names.contains(&field.flattened_field_name.as_str())
                || field_names.contains(&last)
            {
                let is_lat = last == "lat" || last == "ref_lat" || last == "latitude_deg";
                locs.push(FieldLoc {
                    name: field.flattened_field_name.clone(),
                    field_type: field.field_type.clone(),
                    offset: field.offset,
                    is_lat,
                });
            }
        }
        if !locs.is_empty() {
            scrub.borrow_mut().insert(msg_id, locs);
        }
    };
    parser.set_data_message_callback(&mut cb);

    let mut buf = [0u8; 256 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        // A parse error stops us, but we already have everything up to that
        // point which is all we need for the scrub map.
        if parser.consume_bytes(&buf[..n]).is_err() {
            break;
        }
    }

    Ok(scrub.into_inner())
}

/// Apply `delta_deg` to a coordinate encoded as the given type.
/// int32_t: microdegrees * 1e-7 (PX4 convention).
/// double/float: degrees directly.
fn apply_offset(bytes: &mut [u8], field_type: &FlattenedFieldType, delta_deg: f64) {
    match field_type {
        FlattenedFieldType::Int32 => {
            let val = i32::from_le_bytes(bytes.try_into().unwrap());
            let shifted = (val as f64) + delta_deg * 1e7;
            let clamped = shifted.clamp(i32::MIN as f64, i32::MAX as f64) as i32;
            bytes.copy_from_slice(&clamped.to_le_bytes());
        }
        FlattenedFieldType::Double => {
            let val = f64::from_le_bytes(bytes.try_into().unwrap());
            let shifted = val + delta_deg;
            bytes.copy_from_slice(&shifted.to_le_bytes());
        }
        FlattenedFieldType::Float => {
            let val = f32::from_le_bytes(bytes.try_into().unwrap());
            let shifted = (val as f64) + delta_deg;
            bytes.copy_from_slice(&(shifted as f32).to_le_bytes());
        }
        other => {
            eprintln!(
                "warning: unexpected field type {:?} for coordinate, skipping",
                other
            );
        }
    }
}

fn field_width(ft: &FlattenedFieldType) -> usize {
    match ft {
        FlattenedFieldType::Int8
        | FlattenedFieldType::UInt8
        | FlattenedFieldType::Bool
        | FlattenedFieldType::Char => 1,
        FlattenedFieldType::Int16 | FlattenedFieldType::UInt16 => 2,
        FlattenedFieldType::Int32 | FlattenedFieldType::UInt32 | FlattenedFieldType::Float => 4,
        FlattenedFieldType::Int64 | FlattenedFieldType::UInt64 | FlattenedFieldType::Double => 8,
    }
}

/// Mutate a Data message payload in-place for every field in `locs`.
/// `payload` is the full message body INCLUDING the 2-byte msg_id prefix.
fn scrub_data_payload(payload: &mut [u8], locs: &[FieldLoc], dlat: f64, dlon: f64) {
    for loc in locs {
        let width = field_width(&loc.field_type);
        let start = loc.offset as usize;
        let end = start + width;
        if end > payload.len() {
            continue; // corrupted, skip
        }
        let delta = if loc.is_lat { dlat } else { dlon };
        apply_offset(&mut payload[start..end], &loc.field_type, delta);
    }
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        eprintln!("usage: scrub_gps <in.ulg> <out.ulg> <dlat_deg> <dlon_deg>");
        eprintln!();
        eprintln!("Pick any non-trivial offset (say between 0.5 and 10 degrees in each");
        eprintln!("axis). The offset is NOT stored anywhere; once you discard it the");
        eprintln!("original coordinates cannot be recovered from the scrubbed file.");
        std::process::exit(2);
    }
    let in_path = &args[1];
    let out_path = &args[2];
    let dlat: f64 = args[3]
        .parse()
        .map_err(|e| std::io::Error::other(format!("invalid dlat_deg: {}", e)))?;
    let dlon: f64 = args[4]
        .parse()
        .map_err(|e| std::io::Error::other(format!("invalid dlon_deg: {}", e)))?;

    // --- Pass 1: learn the layout.
    eprintln!("pass 1: collecting position-topic layout from {}", in_path);
    let scrub_map = collect_position_layout(in_path)?;
    eprintln!("  scrubbing {} topic(s):", scrub_map.len());
    for (msg_id, locs) in &scrub_map {
        let names: Vec<&str> = locs.iter().map(|l| l.name.as_str()).collect();
        eprintln!("    msg_id={} fields={:?}", msg_id, names);
    }

    // --- Pass 2: copy bytes, mutate on position-topic Data messages.
    eprintln!("pass 2: rewriting file to {}", out_path);
    let bytes = std::fs::read(in_path)?;
    let mut out = Vec::with_capacity(bytes.len());

    if bytes.len() < 16 {
        return Err(std::io::Error::other("file too small for header"));
    }
    // Copy header but zero the top-level timestamp (bytes 8..16).
    out.extend_from_slice(&bytes[0..8]);
    out.extend_from_slice(&[0u8; 8]);

    let mut i = 16usize;
    while i < bytes.len() {
        // Incomplete or truncated message header/body: copy the rest verbatim.
        if i + 3 > bytes.len() {
            out.extend_from_slice(&bytes[i..]);
            break;
        }
        let msg_size = u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
        let msg_type = bytes[i + 2];
        let total = 3 + msg_size;
        if i + total > bytes.len() {
            out.extend_from_slice(&bytes[i..]);
            break;
        }

        let payload_start = i + 3;
        let payload_end = i + total;

        // Emit size + type unchanged.
        out.extend_from_slice(&bytes[i..payload_start]);

        if msg_type == b'D' && msg_size >= 2 {
            let msg_id = u16::from_le_bytes([bytes[payload_start], bytes[payload_start + 1]]);
            if let Some(locs) = scrub_map.get(&msg_id) {
                let mut payload = bytes[payload_start..payload_end].to_vec();
                scrub_data_payload(&mut payload, locs, dlat, dlon);
                out.extend_from_slice(&payload);
                i = payload_end;
                continue;
            }
        }
        out.extend_from_slice(&bytes[payload_start..payload_end]);
        i = payload_end;
    }

    let mut f = File::create(out_path)?;
    f.write_all(&out)?;

    eprintln!("done. {} bytes in, {} bytes out.", bytes.len(), out.len());
    Ok(())
}
