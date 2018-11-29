use std::fs::File;
use std::io::prelude::*;
use std::io::SeekFrom;

const HEADER_BYTES: [u8; 7] = [85, 76, 111, 103, 1, 18, 53];

pub trait ULogHeader {
    fn is_ulog(&mut self) -> bool;
}

impl ULogHeader for File {
    /// Validates that the file is a ulog file with a valid header
    ///
    /// # Examples
    /// ```
    /// use px4_ulog::parser::header::*;
    ///
    /// let filename = format!("{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg", env!("CARGO_MANIFEST_DIR"));
    /// let mut log_file = std::fs::File::open(&filename).unwrap();
    /// assert!(log_file.is_ulog());
    /// ```
    fn is_ulog(&mut self) -> bool {
        self.seek(SeekFrom::Start(0))
            .expect("File must be seekable");
        let mut buffer = [0; 7];
        if let Ok(bytes) = self.read(&mut buffer) {
            bytes == 7 && buffer == HEADER_BYTES
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_does_not_validate_incorrect_file() {
        let filename = format!(
            "{}/tests/fixtures/not_a_log_file.txt",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut log_file = std::fs::File::open(&filename).unwrap();
        assert!(!log_file.is_ulog());
    }

    #[test]
    fn it_seeks_to_the_beginning_when_validating() {
        let filename = format!(
            "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
            env!("CARGO_MANIFEST_DIR")
        );
        let mut log_file = std::fs::File::open(&filename).unwrap();
        log_file.seek(SeekFrom::Start(8)).unwrap();
        assert!(log_file.is_ulog());
    }
}
