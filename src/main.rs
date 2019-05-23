extern crate px4_ulog;

use px4_ulog::models::ULogData;
use px4_ulog::parser::dataset::*;
use std::collections::HashSet;
use std::fs::File;

fn main() {
    let mut args = std::env::args();
    let cmd = args.next();
    if let Some(filename) = args.next() {
        let mut log_file = File::open(&filename).unwrap();

        if let Some(dataset_name) = args.next() {
            let datasets: Vec<ULogData> = log_file.get_dataset(&dataset_name).unwrap().collect();

            println!("Measurements: {}", datasets.len());

            let filters = args.collect::<HashSet<String>>();

            for dataset in datasets.iter() {
                println!("--------------------------");
                for item in dataset.iter() {
                    if filters.len() == 0 || filters.contains(item.name()) {
                        println!("{} at {}: {:?}", item.name(), item.index(), item.data());
                    }
                }
            }
        } else {
            let messages = log_file.get_message_names().unwrap();
            println!("Messages: {}", messages.len());
            for msg in messages {
                println!("{}", msg);
            }
        }
    } else {
        eprintln!(
            "usage: {} log-file.ulg [dataset] [list of filters]",
            cmd.unwrap_or("px4-ulog".to_string())
        );
    }
}
