use std::fs::File;
use std::io::prelude::*;
use std::io::{Error, ErrorKind, Result, SeekFrom};
use std::str;

use super::message::*;
use crate::models::{MessageType, ULogData, ULogMessage};
use crate::unpack;

/// A pointer to a dataset in the log file
pub struct ULogDataset<'a> {
    messages: Vec<ULogMessage>,
    formats: Vec<String>,
    msg_id: u16,
    file: &'a mut File,
    name: &'a str,
}

impl<'a> ULogDataset<'a> {
    pub fn new(messages: Vec<ULogMessage>, file: &'a mut File, name: &'a str) -> Self {
        Self {
            messages,
            formats: Vec::new(),
            msg_id: 0,
            file,
            name,
        }
    }
}

pub trait ULogDatasetSource<'a> {
    /// Get a dataset from the log file
    ///
    /// # Examples
    /// ```
    /// use std::fs::File;
    /// use px4_ulog::models::*;
    /// use px4_ulog::parser::dataset::*;
    ///
    /// let filename = format!("{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg", env!("CARGO_MANIFEST_DIR"));
    /// let mut log_file = File::open(&filename).unwrap();
    ///  
    /// let gps_positions: Vec<ULogData> = log_file
    ///     .get_dataset("vehicle_gps_position")
    ///     .unwrap()
    ///     .collect();
    /// assert_eq!(gps_positions.len(), 260);
    /// ```
    fn get_dataset(&'a mut self, name: &'a str) -> Result<ULogDataset<'a>>;
}

impl<'a> ULogDatasetSource<'a> for File {
    fn get_dataset(&'a mut self, name: &'a str) -> Result<ULogDataset<'a>> {
        let messages: Vec<ULogMessage> = self.messages().collect();
        let set = ULogDataset::new(messages, self, name);
        Ok(set)
    }
}

impl<'a> Iterator for ULogDataset<'a> {
    type Item = ULogData;

    fn next(&mut self) -> Option<Self::Item> {
        let data = get_next_data(self);

        if let Ok(item) = data {
            Some(item)
        } else {
            None
        }
    }
}

fn get_next_data(dataset: &mut ULogDataset) -> Result<ULogData> {
    while dataset.messages.len() > 0 {
        let message = dataset.messages.remove(0);
        match message.msg_type() {
            MessageType::Format => {
                let (format_name, mut types) = parse_format(dataset.file, &message)?;

                if format_name == dataset.name {
                    dataset.formats.append(&mut types);
                }
            }
            MessageType::AddLoggedMessage => {
                let data = read_data(dataset.file, &message)?;
                let message_name = unpack::as_str(&data[3..])?;

                if message_name == dataset.name {
                    //let multi_id = data[0];
                    let mut msg_id_data: [u8; 2] = Default::default();
                    msg_id_data.copy_from_slice(&data[1..3]);
                    dataset.msg_id = unpack::as_u16_le(&msg_id_data);
                }
            }
            MessageType::Data => {
                let data = read_data(dataset.file, &message)?;
                let mut msg_id_data: [u8; 2] = Default::default();
                msg_id_data.copy_from_slice(&data[0..2]);

                let data_msg_id = unpack::as_u16_le(&msg_id_data);

                if data_msg_id == dataset.msg_id {
                    let ulog_data = ULogData::new(data[2..].to_vec(), dataset.formats.clone());
                    return Ok(ulog_data);
                }
            }

            _ => (),
        }
    }
    Err(Error::new(ErrorKind::Other, "no more data"))
}

fn read_data(file: &mut File, message: &ULogMessage) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(message.position()))?;
    let mut handle = file.take(message.size() as u64);
    let mut buffer = Vec::new();
    let bytes = handle.read_to_end(&mut buffer)?;

    if bytes as u16 != message.size() {
        return Err(Error::new(ErrorKind::Other, "unable to read message"));
    }

    Ok(buffer)
}

fn parse_format(file: &mut File, message: &ULogMessage) -> Result<(String, Vec<String>)> {
    let data = read_data(file, message)?;
    let format = std::str::from_utf8(&data)
        .map_err(|_| Error::new(ErrorKind::Other, "format message is not a string"))?;

    let parts: Vec<&str> = format.split(":").collect();

    if parts.len() != 2 {
        return Err(Error::new(ErrorKind::Other, "invalid format string"));
    }

    let name = parts[0].to_string();
    let types: Vec<String> = parts[1].split(";").map(|s| s.to_string()).collect();

    Ok((name, types))
}
