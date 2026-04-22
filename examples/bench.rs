use std::time::Instant;
use px4_ulog::stream_parser::file_reader::{read_file_with_simple_callback, SimpleCallbackResult};

fn bench_file(path: &str) -> (usize, f64, f64) {
    let file_size = std::fs::metadata(path).unwrap().len() as usize;

    // Warmup
    for _ in 0..3 {
        let mut count = 0usize;
        read_file_with_simple_callback(path, &mut |_| {
            count += 1;
            SimpleCallbackResult::KeepReading
        }).unwrap();
    }

    // Measure 10 iterations
    let mut times = Vec::new();
    let mut msg_count = 0usize;
    for _ in 0..10 {
        let mut count = 0usize;
        let start = Instant::now();
        read_file_with_simple_callback(path, &mut |_| {
            count += 1;
            SimpleCallbackResult::KeepReading
        }).unwrap();
        let elapsed = start.elapsed().as_secs_f64();
        times.push(elapsed);
        msg_count = count;
    }

    let mean = times.iter().sum::<f64>() / times.len() as f64;
    let throughput_mbs = (file_size as f64 / (1024.0 * 1024.0)) / mean;

    (msg_count, mean * 1000.0, throughput_mbs)
}

fn main() {
    let fixtures = [
        "tests/fixtures/sample.ulg",
        "tests/fixtures/quadrotor_local.ulg",
        "tests/fixtures/fixed_wing_gps.ulg",
        "tests/fixtures/vtol_demo.ulg",
        "tests/fixtures/truncated_real.ulg",
        "tests/fixtures/sample_appended.ulg",
    ];

    println!("{:<45} {:>8} {:>10} {:>10} {:>12}", "File", "Size", "Messages", "Time(ms)", "MB/s");
    println!("{}", "-".repeat(90));

    let mut total_bytes = 0u64;
    let mut total_time = 0f64;

    for path in &fixtures {
        let size = std::fs::metadata(path).unwrap().len();
        let (msgs, time_ms, throughput) = bench_file(path);
        let size_mb = size as f64 / (1024.0 * 1024.0);
        println!("{:<45} {:>7.1}M {:>10} {:>9.2}ms {:>10.1} MB/s",
            path, size_mb, msgs, time_ms, throughput);
        total_bytes += size;
        total_time += time_ms;
    }

    let total_mb = total_bytes as f64 / (1024.0 * 1024.0);
    println!("{}", "-".repeat(90));
    println!("{:<45} {:>7.1}M {:>10} {:>9.2}ms {:>10.1} MB/s",
        "TOTAL", total_mb, "", total_time, total_mb / (total_time / 1000.0));

    // Also bench the full_parser
    println!("\n--- full_parser::read_file ---");
    for path in &["tests/fixtures/fixed_wing_gps.ulg", "tests/fixtures/sample.ulg"] {
        let size = std::fs::metadata(path).unwrap().len();
        // Warmup
        for _ in 0..3 {
            let _ = px4_ulog::full_parser::read_file(path);
        }
        let mut times = Vec::new();
        for _ in 0..10 {
            let start = Instant::now();
            let _ = px4_ulog::full_parser::read_file(path);
            times.push(start.elapsed().as_secs_f64());
        }
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        let throughput = (size as f64 / (1024.0 * 1024.0)) / mean;
        println!("{:<45} {:>9.2}ms {:>10.1} MB/s",
            path, mean * 1000.0, throughput);
    }
}
