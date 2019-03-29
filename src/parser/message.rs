use crate::models::ULogMessage;
use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;

use crate::unpack;

const HEADER_SIZE: u64 = 16;

pub trait ULogMessageSource {
    /// Creates an iterator that reads through every message in the log file
    ///
    /// # Examples
    /// ```
    /// use std::iter::*;
    /// use px4_ulog::parser::message::*;
    /// use px4_ulog::models::*;
    ///
    /// let filename = format!("{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg", env!("CARGO_MANIFEST_DIR"));
    /// let mut log_file = std::fs::File::open(&filename).unwrap();
    /// let messages: Vec<ULogMessage> = log_file.messages().collect();
    /// assert_eq!(messages[0].position(), 19);
    /// assert_eq!(messages[0].msg_type(), MessageType::FlagBits);
    /// assert_eq!(messages[0].size(), 40);
    /// assert_eq!(messages[1].position(), 62);
    /// assert_eq!(messages[21130].position(), 973045);
    /// assert_eq!(messages.len(), 21131);
    /// ```
    fn messages(&mut self) -> ULogMessageIter;
}

pub struct ULogMessageIter<'a> {
    position: u64,
    file: &'a mut File,
}

impl ULogMessageSource for File {
    fn messages(&mut self) -> ULogMessageIter {
        ULogMessageIter {
            position: HEADER_SIZE,
            file: self,
        }
    }
}

impl<'a> Iterator for ULogMessageIter<'a> {
    type Item = ULogMessage;

    fn next(&mut self) -> Option<ULogMessage> {
        if self.file.seek(SeekFrom::Start(self.position)).is_err() {
            return None;
        }

        let mut buffer = [0; 2];
        if self.file.read_exact(&mut buffer).is_err() {
            return None;
        }
        let msg_size = unpack::as_u16_le(&buffer);

        let mut buffer = [0; 1];
        if self.file.read_exact(&mut buffer).is_err() {
            return None;
        }
        let msg_type = buffer[0];

        let msg_pos = self.position + 3;

        self.position += msg_size as u64 + 3;

        Some(ULogMessage::new(msg_type, msg_size, msg_pos))
    }
}
