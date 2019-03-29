extern crate px4_ulog;

use std::time::Instant;

fn main() {
  let log_file_path = std::env::var("FILE").unwrap();
  let start = Instant::now();
  let bytes_read =
    px4_ulog::full_parser::read_file(&log_file_path).expect("got error during file reading");
  println!(
    "Read the {:#?} messages in {}.{:03} s, file '{}'",
    bytes_read.messages.keys(),
    start.elapsed().as_secs(),
    start.elapsed().subsec_millis(),
    log_file_path
  );
}
