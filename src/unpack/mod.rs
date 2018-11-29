use std::iter::*;

/// Convert a slice of eight u8 elements into a u64
/// Assumes little endianness.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// let arr: [u8; 8] = [7, 6, 5, 4, 3, 2, 1, 0];
/// assert_eq!(unpack::as_u64_le(&arr), 283686952306183);
/// ```
pub fn as_u64_le(arr: &[u8; 8]) -> u64 {
    arr.iter()
        .enumerate()
        .map(|(i, v)| (v.clone() as u64) << (8 * i))
        .sum()
}

/// Convert a slice of two u8 elements into a u16
/// Assumes little endianness.
///
/// # Examples
/// ```
/// use px4_ulog::unpack;
/// let arr: [u8; 2] = [0, 2];
/// assert_eq!(unpack::as_u16_le(&arr), 512);
/// ```
pub fn as_u16_le(arr: &[u8; 2]) -> u16 {
    arr.iter()
        .enumerate()
        .map(|(i, v)| (v.clone() as u16) << (8 * i))
        .sum()
}
