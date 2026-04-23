//! Little-endian byte-slice unpacking helpers.
//!
//! Thin wrappers over stdlib primitives. Callers pass sub-slices taken from
//! the raw ULog byte stream; these helpers handle the slice-to-array conversion
//! so call sites stay compact. Input length is expected to match the target
//! type's byte width; wrong lengths panic (always a parser bug).

use std::io::{Error, ErrorKind, Result};

/// Read a little-endian `u64` from an 8-byte slice.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// assert_eq!(unpack::as_u64_le(&[7, 6, 5, 4, 3, 2, 1, 0]), 283686952306183);
/// ```
pub fn as_u64_le(arr: &[u8]) -> u64 {
    u64::from_le_bytes(arr.try_into().expect("as_u64_le expects 8 bytes"))
}

/// Read a little-endian `u32` from a 4-byte slice.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// assert_eq!(unpack::as_u32_le(&[2, 1, 0, 0]), 258);
/// ```
pub fn as_u32_le(arr: &[u8]) -> u32 {
    u32::from_le_bytes(arr.try_into().expect("as_u32_le expects 4 bytes"))
}

/// Read a little-endian `i32` from a 4-byte slice.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// assert_eq!(unpack::as_i32_le(&[1, 0, 0, 255]), -16777215);
/// ```
pub fn as_i32_le(arr: &[u8]) -> i32 {
    i32::from_le_bytes(arr.try_into().expect("as_i32_le expects 4 bytes"))
}

/// Read a little-endian `u16` from a 2-byte slice.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// assert_eq!(unpack::as_u16_le(&[0, 2]), 512);
/// ```
pub fn as_u16_le(arr: &[u8]) -> u16 {
    u16::from_le_bytes(arr.try_into().expect("as_u16_le expects 2 bytes"))
}

/// Read a little-endian `f32` from a 4-byte slice.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// assert_eq!(unpack::as_f32_le(&[0, 0, 0, 0]), 0.0);
/// ```
pub fn as_f32_le(arr: &[u8]) -> f32 {
    f32::from_le_bytes(arr.try_into().expect("as_f32_le expects 4 bytes"))
}

/// Interpret a byte slice as UTF-8. Returns an I/O error on invalid UTF-8.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// assert_eq!(unpack::as_str(&[72, 101, 108, 108, 111]).unwrap(), "Hello");
/// ```
pub fn as_str(arr: &[u8]) -> Result<&str> {
    std::str::from_utf8(arr).map_err(|_| Error::new(ErrorKind::Other, "data is not a string"))
}
