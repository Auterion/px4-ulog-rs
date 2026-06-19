use px4_ulog::full_parser::{read_file, MultiId, SomeVec};

fn main() {
    let filename = format!(
        "{}/tests/fixtures/6ba1abc7-b433-4029-b8f5-3b2bb12d3b6c.ulg",
        env!("CARGO_MANIFEST_DIR")
    );
    let parsed = read_file(&filename).expect("Failed to parse ULog file");

    let gps = parsed
        .messages
        .get("vehicle_gps_position")
        .and_then(|m| m.get(&MultiId::new(0)))
        .expect("vehicle_gps_position not found");

    let count = gps
        .get("timestamp")
        .map(|v| match v {
            SomeVec::UInt64(v) => v.len(),
            _ => 0,
        })
        .unwrap_or(0);

    println!("Measurements: {}", count);

    for i in 0..count {
        println!("--------------------------");
        for (name, vec) in gps {
            let val = match vec {
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
            };
            println!("{}: {}", name, val);
        }
    }
}
