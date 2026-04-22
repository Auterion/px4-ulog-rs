//! Smoke tests for the unpack helpers. The heavy lifting is in stdlib
//! (`u*::from_le_bytes`), so one representative value per function is enough.
//! f32 special-value safety is covered in regression_bugs.rs.

use px4_ulog::unpack;

#[test]
fn as_u64_le_reads_little_endian() {
    assert_eq!(unpack::as_u64_le(&[7, 6, 5, 4, 3, 2, 1, 0]), 283686952306183u64);
}

#[test]
fn as_u32_le_reads_little_endian() {
    assert_eq!(unpack::as_u32_le(&[2, 1, 0, 0]), 258u32);
}

#[test]
fn as_i32_le_handles_negative() {
    assert_eq!(unpack::as_i32_le(&[0xFF, 0xFF, 0xFF, 0xFF]), -1i32);
}

#[test]
fn as_u16_le_reads_little_endian() {
    assert_eq!(unpack::as_u16_le(&[0, 2]), 512u16);
}

#[test]
fn as_f32_le_round_trips() {
    assert_eq!(
        unpack::as_f32_le(&std::f32::consts::PI.to_le_bytes()),
        std::f32::consts::PI
    );
}

#[test]
fn as_str_accepts_utf8_and_rejects_invalid() {
    assert_eq!(unpack::as_str(b"PX4").unwrap(), "PX4");
    assert!(unpack::as_str(&[0xFF, 0xFE]).is_err());
}
