extern crate px4_ulog;

use px4_ulog::models::ULogData;
use px4_ulog::parser::dataset::*;
use std::fs::File;

fn main() {
    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut log_file = File::open(&filename).unwrap();

    let gps_positions: Vec<ULogData> = log_file
        .get_dataset("vehicle_gps_position")
        .unwrap()
        .collect();

    for data in gps_positions {
        println!("GPS data: {:?}", data);
    }
}
