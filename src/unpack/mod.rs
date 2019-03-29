use std::io::{Error, ErrorKind, Result};
use std::iter::*;

/// Convert a array of eight u8 elements into a u64
/// Assumes little endianness.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// let arr: [u8; 8] = [7, 6, 5, 4, 3, 2, 1, 0];
/// assert_eq!(unpack::as_u64_le(&arr), 283686952306183);
/// ```
pub fn as_u64_le(arr: &[u8]) -> u64 {
        arr.iter()
                .enumerate()
                .map(|(i, v)| (v.clone() as u64) << (8 * i))
                .sum()
}

/// Convert a array of four u8 elements into a u32
/// Assumes little endianness.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// let arr: [u8; 4] = [2, 1, 0, 0];
/// assert_eq!(unpack::as_u32_le(&arr), 258);
/// ```
pub fn as_u32_le(arr: &[u8]) -> u32 {
        arr.iter()
                .enumerate()
                .map(|(i, v)| (v.clone() as u32) << (8 * i))
                .sum()
}

/// Convert a array of four u8 elements into a i32
/// Assumes little endianness.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// let arr: [u8; 4] = [1, 0, 0, 255];
/// assert_eq!(unpack::as_i32_le(&arr), -16777215);
/// ```
pub fn as_i32_le(arr: &[u8]) -> i32 {
        as_u32_le(arr) as i32
}

/// Convert a array of two u8 elements into a u16
/// Assumes little endianness.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// let arr: [u8; 2] = [0, 2];
/// assert_eq!(unpack::as_u16_le(&arr), 512);
/// ```
pub fn as_u16_le(arr: &[u8]) -> u16 {
        arr.iter()
                .enumerate()
                .map(|(i, v)| (v.clone() as u16) << (8 * i))
                .sum()
}

/// Convert a array of four u8 elements into a f32
/// Assumes little endianness.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// let arr: [u8; 4] = [0x0, 0, 0, 0];
/// assert_eq!(unpack::as_f32_le(&arr), 0.0);
/// ```
pub fn as_f32_le(arr: &[u8]) -> f32 {
        unsafe { *(&as_u32_le(arr) as *const u32 as *const f32) }
}

/// Convert a u8 slice to a string
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// let arr: [u8; 5] = [72, 101, 108, 108, 111];
/// assert_eq!(unpack::as_str(&arr).unwrap(), "Hello");
/// ```
pub fn as_str(arr: &[u8]) -> Result<&str> {
        std::str::from_utf8(arr).map_err(|_| Error::new(ErrorKind::Other, "data is not a string"))
}
