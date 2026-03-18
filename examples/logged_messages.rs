extern crate px4_ulog;

use px4_ulog::stream_parser::{LogParser, LoggedStringMessage};
use std::fs::File;
use std::io::Read;

fn main() {
    let mut args = std::env::args();
    let cmd = args.next();

    let Some(filename) = args.next() else {
        eprintln!("usage: {} <log-file.ulg>", cmd.unwrap_or("logged_messages".to_string()));
        eprintln!("\nPrints all logged string messages with their log levels.");
        return;
    };

    let mut f = File::open(&filename).expect("Failed to open file");

    let mut callback = |msg: &LoggedStringMessage| {
        let effective_level = msg.human_readable_log_level();

        println!(
            "[{:.2}s] {}: {}",
            msg.timestamp as f64 / 1_000_000.0,
            effective_level,
            msg.logged_message.trim()
        );
    };

    let mut parser = LogParser::default();
    parser.set_logged_string_message_callback(&mut callback);

    let mut buf = [0u8; 64 * 1024];

    loop {
        let num_bytes_read = f.read(&mut buf).expect("Failed to read file");
        if num_bytes_read == 0 {
            break;
        }
        parser
            .consume_bytes(&buf[..num_bytes_read])
            .expect("Parse error");
    }
}
