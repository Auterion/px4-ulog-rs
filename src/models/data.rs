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

#[derive(Debug)]
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
