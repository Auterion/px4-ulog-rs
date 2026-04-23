use px4_ulog::full_parser::{read_file, SomeVec};
use std::collections::HashSet;

fn main() {
    let mut args = std::env::args();
    let cmd = args.next();
    if let Some(filename) = args.next() {
        let parsed = read_file(&filename).expect("Failed to parse ULog file");

        if let Some(dataset_name) = args.next() {
            let filters = args.collect::<HashSet<String>>();

            if let Some(multi_map) = parsed.messages.get(&dataset_name) {
                for (multi_id, fields) in multi_map {
                    let count = fields.values().next().map(somevec_len).unwrap_or(0);
                    println!(
                        "Topic: {} (multi_id={}), Measurements: {}",
                        dataset_name,
                        multi_id.value(),
                        count
                    );

                    if count > 0 {
                        for i in 0..count {
                            println!("--------------------------");
                            for (name, vec) in fields {
                                if filters.is_empty() || filters.contains(name) {
                                    println!("{} at {}: {:?}", name, i, somevec_get(vec, i));
                                }
                            }
                        }
                    }
                }
            } else {
                eprintln!("Dataset '{}' not found in log", dataset_name);
                eprintln!("Available topics:");
                for name in parsed.messages.keys() {
                    eprintln!("  {}", name);
                }
            }
        } else {
            println!("Topics: {}", parsed.messages.len());
            for (name, multi_map) in &parsed.messages {
                for (multi_id, fields) in multi_map {
                    let count = fields.values().next().map(somevec_len).unwrap_or(0);
                    println!(
                        "  {} (multi_id={}, {} messages, {} fields)",
                        name,
                        multi_id.value(),
                        count,
                        fields.len()
                    );
                }
            }
        }
    } else {
        eprintln!(
            "usage: {} log-file.ulg [dataset] [list of filters]",
            cmd.unwrap_or("px4-ulog".to_string())
        );
    }
}

fn somevec_len(v: &SomeVec) -> usize {
    match v {
        SomeVec::Int8(v) => v.len(),
        SomeVec::UInt8(v) => v.len(),
        SomeVec::Int16(v) => v.len(),
        SomeVec::UInt16(v) => v.len(),
        SomeVec::Int32(v) => v.len(),
        SomeVec::UInt32(v) => v.len(),
        SomeVec::Int64(v) => v.len(),
        SomeVec::UInt64(v) => v.len(),
        SomeVec::Float(v) => v.len(),
        SomeVec::Double(v) => v.len(),
        SomeVec::Bool(v) => v.len(),
        SomeVec::Char(v) => v.len(),
    }
}

fn somevec_get(v: &SomeVec, i: usize) -> String {
    match v {
        SomeVec::Int8(v) => format!("{}", v[i]),
        SomeVec::UInt8(v) => format!("{}", v[i]),
        SomeVec::Int16(v) => format!("{}", v[i]),
        SomeVec::UInt16(v) => format!("{}", v[i]),
        SomeVec::Int32(v) => format!("{}", v[i]),
        SomeVec::UInt32(v) => format!("{}", v[i]),
        SomeVec::Int64(v) => format!("{}", v[i]),
        SomeVec::UInt64(v) => format!("{}", v[i]),
        SomeVec::Float(v) => format!("{}", v[i]),
        SomeVec::Double(v) => format!("{}", v[i]),
        SomeVec::Bool(v) => format!("{}", v[i]),
        SomeVec::Char(v) => format!("{}", v[i]),
    }
}
