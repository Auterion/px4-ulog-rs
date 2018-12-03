use unpack;

#[derive(Debug)]
pub struct ULogData {
    data: Vec<u8>,
    formats: Vec<String>,
}

pub struct ULogDataIter<'a> {
    data: &'a ULogData,
    format_index: usize,
    data_index: usize,
}

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

    pub fn data(&self) -> &Vec<u8> {
        &self.data
    }

    pub fn formats(&self) -> &Vec<String> {
        &self.formats
    }

    pub fn items(&self) -> Vec<String> {
        self.formats
            .iter()
            .filter(|f| f.len() > 0 && !f.starts_with("_padding") && f.contains(" "))
            .map(|f| f.split(" ").last().unwrap().to_string())
            .collect()
    }

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
    use parser::dataset::*;
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

        for (name, data) in first_position.iter() {
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
    }
}
