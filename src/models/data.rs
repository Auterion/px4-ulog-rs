#[derive(Debug)]
pub struct ULogData {
    data: Vec<u8>,
}

impl ULogData {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }
}
