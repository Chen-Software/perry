// JSON pipeline: read 108MB array, filter active records, add 2 derived fields,
// serialize back, write output. Reports a summary line with FNV-1a of the
// serialized output so all three implementations can verify agreement.

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::Write;
use std::process::ExitCode;

#[derive(Deserialize, Serialize)]
struct Addr {
    street: String,
    city: String,
    zip: u32,
}

#[derive(Deserialize)]
struct InRecord {
    id: u64,
    name: String,
    email: String,
    age: u32,
    country: String,
    tags: Vec<String>,
    score: u32,
    active: bool,
    addr: Addr,
}

#[derive(Serialize)]
struct OutRecord {
    id: u64,
    name: String,
    email: String,
    age: u32,
    country: String,
    tags: Vec<String>,
    score: u32,
    active: bool,
    addr: Addr,
    display_name: String,
    age_group: &'static str,
}

fn age_group(age: u32) -> &'static str {
    if age < 30 { "young" } else if age < 50 { "mid" } else { "senior" }
}

fn fnv1a32(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    h
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: {} <input.json> <output.json>", args[0]);
        return ExitCode::from(1);
    }
    let input = match fs::read(&args[1]) {
        Ok(b) => b,
        Err(e) => { eprintln!("read: {}", e); return ExitCode::from(1); }
    };
    let input_bytes = input.len();

    let records: Vec<InRecord> = match serde_json::from_slice(&input) {
        Ok(r) => r,
        Err(e) => { eprintln!("parse: {}", e); return ExitCode::from(1); }
    };
    let records_in = records.len();

    let out: Vec<OutRecord> = records.into_iter()
        .filter(|r| r.active)
        .map(|r| {
            let group = age_group(r.age);
            let display_name = r.name.to_uppercase();
            OutRecord {
                id: r.id,
                name: r.name,
                email: r.email,
                age: r.age,
                country: r.country,
                tags: r.tags,
                score: r.score,
                active: r.active,
                addr: r.addr,
                display_name,
                age_group: group,
            }
        })
        .collect();
    let records_out = out.len();

    let serialized = match serde_json::to_vec(&out) {
        Ok(v) => v,
        Err(e) => { eprintln!("serialize: {}", e); return ExitCode::from(1); }
    };
    let output_bytes = serialized.len();

    if let Err(e) = fs::write(&args[2], &serialized) {
        eprintln!("write: {}", e);
        return ExitCode::from(1);
    }
    let hash = fnv1a32(&serialized);

    let _ = writeln!(
        std::io::stdout(),
        "input_bytes={} records_in={} records_out={} output_bytes={} hash={:08x}",
        input_bytes, records_in, records_out, output_bytes, hash
    );
    ExitCode::from(0)
}
