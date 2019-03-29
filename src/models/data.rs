use crate::unpack;

/// Container for a single data row
#[derive(Debug)]
pub struct ULogData {
    data: Vec<u8>,
    formats: Vec<String>,
}

/// Data set iterator
///
/// # Examples
/// ```
/// use std::fs::File;
/// use px4_ulog::parser::dataset::*;
/// let filename = format!(
///     "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
///     env!("CARGO_MANIFEST_DIR")
/// );
///
/// let mut log_file = File::open(&filename).unwrap();
///
/// let starting_gps_position = log_file
///     .get_dataset("vehicle_gps_position")
///     .unwrap()
///     .next()
///     .unwrap();
/// assert_eq!(starting_gps_position.iter().count(), 23);
/// ```
pub struct ULogDataIter<'a> {
    data: &'a ULogData,
    format_index: usize,
    data_index: usize,
}

/// Log data item type
#[derive(Debug, PartialEq)]
pub enum DataType {
    UInt64(u64),
    Int32(i32),
    Float(f32),
    UInt8(u8),
    Bool(bool),
}

impl ULogData {
    pub fn new(data: Vec<u8>, formats: Vec<String>) -> Self {
        Self { data, formats }
    }

    /// Get the unformatted data for this item
    pub fn data(&self) -> &Vec<u8> {
        &self.data
    }

    /// Get the data formatting for this item
    pub fn formats(&self) -> &Vec<String> {
        &self.formats
    }

    /// Get the list of field names in this item
    ///
    /// # Examples
    /// ```
    /// use std::fs::File;
    /// use px4_ulog::parser::dataset::*;
    /// let filename = format!(
    ///     "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
    ///     env!("CARGO_MANIFEST_DIR")
    /// );
    ///
    /// let mut log_file = File::open(&filename).unwrap();
    /// let items = log_file.get_dataset("vehicle_gps_position")
    ///     .unwrap()
    ///     .next()
    ///     .unwrap()
    ///     .items();
    /// assert_eq!(items[0], "timestamp");
    /// assert_eq!(items[1], "time_utc_usec");
    /// assert_eq!(items.len(), 23);
    /// ```
    pub fn items(&self) -> Vec<String> {
        self.formats
            .iter()
            .filter(|f| f.len() > 0 && !f.contains("_padding") && f.contains(" "))
            .map(|f| f.split(" ").last().unwrap().to_string())
            .collect()
    }

    /// Get an iterator for data fields
    ///
    /// The iterator value will be a tuple of (&str, DataType)
    /// where the first item will be the field name and the second the value.
    ///
    /// # Examples
    /// ```
    /// use std::fs::File;
    /// use px4_ulog::parser::dataset::*;
    /// let filename = format!(
    ///     "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
    ///     env!("CARGO_MANIFEST_DIR")
    /// );
    ///
    /// let mut log_file = File::open(&filename).unwrap();
    /// let mut dataset = log_file.get_dataset("vehicle_gps_position").unwrap();
    /// let first_data = dataset.next().unwrap();
    /// assert_eq!(first_data.iter().count(), 23);
    /// ```
    pub fn iter(&self) -> ULogDataIter {
        ULogDataIter {
            data: self,
            format_index: 0,
            data_index: 0,
        }
    }
}

impl<'a> Iterator for ULogDataIter<'a> {
    type Item = (&'a str, DataType);

    fn next(&mut self) -> Option<Self::Item> {
        if self.format_index > self.data.formats.len() || self.data_index >= self.data.data.len() {
            None
        } else {
            let format = &self.data.formats[self.format_index];
            self.format_index += 1;
            let space = format.find(" ").unwrap();
            let (dtype, fname) = format.split_at(space);
            let fname = fname.trim();

            match dtype {
                "uint64_t" => {
                    let data_to = self.data_index + 8;
                    let val = if self.data.data.len() > data_to {
                        let mut buf: [u8; 8] = Default::default();
                        buf.copy_from_slice(&self.data.data[self.data_index..data_to]);
                        self.data_index += 8;
                        unpack::as_u64_le(&buf)
                    } else {
                        0
                    };
                    Some((fname, DataType::UInt64(val)))
                }
                "int32_t" => {
                    let data_to = self.data_index + 4;
                    let val = if self.data.data.len() > data_to {
                        let mut buf: [u8; 4] = Default::default();
                        buf.copy_from_slice(&self.data.data[self.data_index..data_to]);
                        self.data_index += 4;
                        unpack::as_i32_le(&buf)
                    } else {
                        0
                    };
                    Some((fname, DataType::Int32(val)))
                }
                "float" => {
                    let data_to = self.data_index + 4;
                    let val = if self.data.data.len() > data_to {
                        let mut buf: [u8; 4] = Default::default();
                        buf.copy_from_slice(&self.data.data[self.data_index..data_to]);
                        self.data_index += 4;
                        unpack::as_f32_le(&buf)
                    } else {
                        0.0
                    };
                    Some((fname, DataType::Float(val)))
                }
                "uint8_t" => {
                    let val = if self.data.data.len() > self.data_index {
                        let v = self.data.data[self.data_index];
                        self.data_index += 1;
                        v
                    } else {
                        0
                    };
                    Some((fname, DataType::UInt8(val)))
                }
                "bool" => {
                    let val = if self.data.data.len() > self.data_index {
                        let v = self.data.data[self.data_index] > 0;
                        self.data_index += 1;
                        v
                    } else {
                        false
                    };
                    Some((fname, DataType::Bool(val)))
                }
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::dataset::*;
    use std::collections::HashMap;
    use std::fs::File;

    #[test]
    fn it_parses_the_data() {
        let filename = format!(
            "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
            env!("CARGO_MANIFEST_DIR")
        );

        let mut log_file = File::open(&filename).unwrap();

        let first_position = log_file
            .get_dataset("vehicle_gps_position")
            .unwrap()
            .next()
            .unwrap();

        let items = first_position.items();
        let mut seen = HashMap::new();
        for item in items.clone() {
            seen.insert(item.clone(), 0);
        }

        for (name, data) in first_position.iter() {
            *seen.get_mut(name).unwrap() += 1;
            match name {
                "timestamp" => assert_eq!(DataType::UInt64(375408345), data),
                "time_utc_usec" => assert_eq!(DataType::UInt64(0), data),
                "lat" => assert_eq!(DataType::Int32(407423012), data),
                "lon" => assert_eq!(DataType::Int32(-741792999), data),
                "alt" => assert_eq!(DataType::Int32(28495), data),
                "alt_ellipsoid" => assert_eq!(DataType::Int32(0), data),
                "s_variance_m_s" => assert_eq!(DataType::Float(0.0), data),
                "c_variance_rad" => assert_eq!(DataType::Float(0.0), data),
                "eph" => assert_eq!(DataType::Float(0.29999998), data),
                "epv" => assert_eq!(DataType::Float(0.39999998), data),
                "hdop" => assert_eq!(DataType::Float(0.0), data),
                "vdop" => assert_eq!(DataType::Float(0.0), data),
                "noise_per_ms" => assert_eq!(DataType::Int32(0), data),
                "jamming_indicator" => assert_eq!(DataType::Int32(0), data),
                "vel_m_s" => assert_eq!(DataType::Float(0.0), data),
                "vel_n_m_s" => assert_eq!(DataType::Float(0.0), data),
                "vel_e_m_s" => assert_eq!(DataType::Float(0.0), data),
                "vel_d_m_s" => assert_eq!(DataType::Float(0.0), data),
                "cog_rad" => assert_eq!(DataType::Float(0.0), data),
                "timestamp_time_relative" => assert_eq!(DataType::Int32(0), data),
                "fix_type" => assert_eq!(DataType::UInt8(3), data),
                "vel_ned_valid" => assert_eq!(DataType::Bool(false), data),
                "satellites_used" => assert_eq!(DataType::UInt8(10), data),
                x => panic!(format!("unexpected field '{}'", x)),
            }
        }

        for item in items {
            assert_eq!(seen.get(item.as_str()), Some(&1), "item {} not seen", item);
        }
    }
}
