// Parse every ULog file reachable from a path list or a directory walk and
// report the outcome per file. Useful for corpus regression testing and for
// cross-checking against another parser (pair with scripts/ulog_cpp_check
// and diff the outputs).
//
// Input modes:
//   scan_corpus <dir>              walk <dir> recursively, parse every *.ulg
//   scan_corpus --list <paths.txt> parse one path per line from the file
//   scan_corpus --stdin            parse one path per line from stdin
//
// Per-file output (tab-separated, stdout):
//   OK\t<path>                                  parse returned Ok
//   ERR\t<path>\t<one-line error message>       parse returned Err (or panic)
//
// A summary with file counts, byte counts and elapsed time goes to stderr.

use px4_ulog::stream_parser::file_reader::LogParser;
use px4_ulog::stream_parser::model::DataMessage;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::time::Instant;

enum Outcome {
    Ok { bytes: u64 },
    Err(String),
}

fn parse_one(path: &Path) -> Outcome {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut f = File::open(path).map_err(|e| format!("open: {}", e))?;
        let mut parser = LogParser::default();
        let mut noop = |_: &DataMessage| {};
        parser.set_data_message_callback(&mut noop);

        let mut bytes_read: u64 = 0;
        let mut buf = [0u8; 256 * 1024];
        loop {
            match f.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    bytes_read += n as u64;
                    parser
                        .consume_bytes(&buf[..n])
                        .map_err(|e| format!("{}", e))?;
                }
                Err(e) => return Err(format!("read: {}", e)),
            }
        }
        Ok::<u64, String>(bytes_read)
    }));

    match result {
        Ok(Ok(bytes)) => Outcome::Ok { bytes },
        Ok(Err(e)) => Outcome::Err(one_line(&e)),
        Err(panic) => {
            let msg = if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic.downcast_ref::<&str>() {
                (*s).to_string()
            } else {
                "<non-string panic>".to_string()
            };
            Outcome::Err(format!("panic: {}", one_line(&msg)))
        }
    }
}

fn one_line(s: &str) -> String {
    s.chars().map(|c| if c == '\n' { ' ' } else { c }).collect()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map(|e| e == "ulg").unwrap_or(false) {
                out.push(p);
            }
        }
    }
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  \
         scan_corpus <dir>              walk <dir> for *.ulg\n  \
         scan_corpus --list <paths.txt> read one path per line from file\n  \
         scan_corpus --stdin            read one path per line from stdin"
    );
    std::process::exit(2);
}

fn collect_paths() -> Vec<PathBuf> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--list") => {
            let file = args.next().unwrap_or_else(|| usage());
            let reader = BufReader::new(File::open(&file).expect("open list file"));
            reader
                .lines()
                .map_while(Result::ok)
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .map(PathBuf::from)
                .collect()
        }
        Some("--stdin") => {
            let stdin = std::io::stdin();
            stdin
                .lock()
                .lines()
                .map_while(Result::ok)
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .map(PathBuf::from)
                .collect()
        }
        Some(dir) if !dir.starts_with("--") => {
            let mut out = Vec::new();
            walk(Path::new(dir), &mut out);
            out.sort();
            out
        }
        _ => usage(),
    }
}

fn main() {
    let files = collect_paths();
    eprintln!("scanning {} file(s)...", files.len());

    let start = Instant::now();
    let mut n_ok = 0u64;
    let mut n_err = 0u64;
    let mut total_bytes = 0u64;

    for path in &files {
        match parse_one(path) {
            Outcome::Ok { bytes } => {
                n_ok += 1;
                total_bytes += bytes;
                println!("OK\t{}", path.display());
            }
            Outcome::Err(msg) => {
                n_err += 1;
                println!("ERR\t{}\t{}", path.display(), msg);
            }
        }
    }

    let elapsed = start.elapsed();
    eprintln!(
        "{} ok, {} err; {:.1} MB parsed in {:.1}s",
        n_ok,
        n_err,
        total_bytes as f64 / 1024.0 / 1024.0,
        elapsed.as_secs_f64()
    );
}
