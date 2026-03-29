//! Priority 2c: Comprehensive unit tests for the unpack module.

use px4_ulog::unpack;

// =============================================================================
// u64
// =============================================================================

#[test]
fn test_u64_zero() {
    assert_eq!(unpack::as_u64_le(&[0, 0, 0, 0, 0, 0, 0, 0]), 0u64);
}

#[test]
fn test_u64_one() {
    assert_eq!(unpack::as_u64_le(&[1, 0, 0, 0, 0, 0, 0, 0]), 1u64);
}

#[test]
fn test_u64_max() {
    assert_eq!(
        unpack::as_u64_le(&[0xFF; 8]),
        u64::MAX
    );
}

#[test]
fn test_u64_known_value() {
    // 283686952306183 from doc-test
    assert_eq!(
        unpack::as_u64_le(&[7, 6, 5, 4, 3, 2, 1, 0]),
        283686952306183u64
    );
}

#[test]
fn test_u64_le_byte_order() {
    // 0x0100000000000000 in LE = byte 0 is LSB
    assert_eq!(
        unpack::as_u64_le(&[0, 0, 0, 0, 0, 0, 0, 1]),
        0x0100000000000000u64
    );
}

// =============================================================================
// u32
// =============================================================================

#[test]
fn test_u32_zero() {
    assert_eq!(unpack::as_u32_le(&[0, 0, 0, 0]), 0u32);
}

#[test]
fn test_u32_one() {
    assert_eq!(unpack::as_u32_le(&[1, 0, 0, 0]), 1u32);
}

#[test]
fn test_u32_max() {
    assert_eq!(unpack::as_u32_le(&[0xFF; 4]), u32::MAX);
}

#[test]
fn test_u32_258() {
    // From doc-test
    assert_eq!(unpack::as_u32_le(&[2, 1, 0, 0]), 258u32);
}

#[test]
fn test_u32_256() {
    assert_eq!(unpack::as_u32_le(&[0, 1, 0, 0]), 256u32);
}

// =============================================================================
// i32
// =============================================================================

#[test]
fn test_i32_zero() {
    assert_eq!(unpack::as_i32_le(&[0, 0, 0, 0]), 0i32);
}

#[test]
fn test_i32_minus_one() {
    assert_eq!(unpack::as_i32_le(&[0xFF, 0xFF, 0xFF, 0xFF]), -1i32);
}

#[test]
fn test_i32_min() {
    assert_eq!(
        unpack::as_i32_le(&[0x00, 0x00, 0x00, 0x80]),
        i32::MIN
    );
}

#[test]
fn test_i32_max() {
    assert_eq!(
        unpack::as_i32_le(&[0xFF, 0xFF, 0xFF, 0x7F]),
        i32::MAX
    );
}

#[test]
fn test_i32_known_value() {
    // From doc-test: -16777215
    assert_eq!(unpack::as_i32_le(&[1, 0, 0, 255]), -16777215i32);
}

// =============================================================================
// u16
// =============================================================================

#[test]
fn test_u16_zero() {
    assert_eq!(unpack::as_u16_le(&[0, 0]), 0u16);
}

#[test]
fn test_u16_one() {
    assert_eq!(unpack::as_u16_le(&[1, 0]), 1u16);
}

#[test]
fn test_u16_max() {
    assert_eq!(unpack::as_u16_le(&[0xFF, 0xFF]), u16::MAX);
}

#[test]
fn test_u16_256() {
    assert_eq!(unpack::as_u16_le(&[0, 1]), 256u16);
}

#[test]
fn test_u16_512() {
    // From doc-test
    assert_eq!(unpack::as_u16_le(&[0, 2]), 512u16);
}

// =============================================================================
// f32
// =============================================================================

#[test]
fn test_f32_zero() {
    assert_eq!(unpack::as_f32_le(&[0, 0, 0, 0]), 0.0f32);
}

#[test]
fn test_f32_one() {
    assert_eq!(unpack::as_f32_le(&1.0f32.to_le_bytes()), 1.0f32);
}

#[test]
fn test_f32_negative() {
    assert_eq!(unpack::as_f32_le(&(-1.0f32).to_le_bytes()), -1.0f32);
}

#[test]
fn test_f32_pi() {
    assert_eq!(
        unpack::as_f32_le(&std::f32::consts::PI.to_le_bytes()),
        std::f32::consts::PI
    );
}

#[test]
fn test_f32_nan() {
    assert!(unpack::as_f32_le(&f32::NAN.to_le_bytes()).is_nan());
}

#[test]
fn test_f32_infinity() {
    assert_eq!(
        unpack::as_f32_le(&f32::INFINITY.to_le_bytes()),
        f32::INFINITY
    );
}

#[test]
fn test_f32_matches_from_bits() {
    // Exhaustive check of special bit patterns
    for bits in [
        0x00000000u32, // +0
        0x80000000,    // -0
        0x3F800000,    // 1.0
        0xBF800000,    // -1.0
        0x7F800000,    // +inf
        0xFF800000,    // -inf
        0x7FC00000,    // NaN
        0x00000001,    // smallest subnormal
        0x7F7FFFFF,    // largest normal (f32::MAX)
        0x00800000,    // smallest normal
        0x42280000,    // 42.0
    ] {
        let from_unpack = unpack::as_f32_le(&bits.to_le_bytes());
        let from_std = f32::from_bits(bits);
        assert_eq!(
            from_unpack.to_bits(),
            from_std.to_bits(),
            "Bit pattern 0x{:08X}",
            bits
        );
    }
}

// =============================================================================
// str
// =============================================================================

#[test]
fn test_str_hello() {
    assert_eq!(unpack::as_str(&[72, 101, 108, 108, 111]).unwrap(), "Hello");
}

#[test]
fn test_str_empty() {
    assert_eq!(unpack::as_str(&[]).unwrap(), "");
}

#[test]
fn test_str_invalid_utf8() {
    assert!(unpack::as_str(&[0xFF, 0xFE]).is_err());
}

#[test]
fn test_str_ascii() {
    assert_eq!(unpack::as_str(b"PX4").unwrap(), "PX4");
}
