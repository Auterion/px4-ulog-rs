use std::iter::*;

/// Convert a slice of 8 u8 elements into a u64
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
