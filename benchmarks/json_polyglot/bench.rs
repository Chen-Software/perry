// JSON parse + stringify polyglot benchmark — Rust (serde_json).
// 10k records, ~1 MB blob, 50 iterations.
// IDENTICAL workload to bench.ts / bench.go / bench.swift / bench.cpp / bench.js.
//
// To build: a Cargo.toml is provided alongside this file; run.sh invokes
// `cargo build --release` (or `--profile release-aggressive` with the
// flags table from benchmarks/README.md).

use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Serialize, Deserialize)]
struct Nested {
    x: i64,
    y: i64,
}

#[derive(Serialize, Deserialize)]
struct Item {
    id: i64,
    name: String,
    value: f64,
    tags: Vec<String>,
    nested: Nested,
}

fn main() {
    let mut items: Vec<Item> = Vec::with_capacity(10_000);
    for i in 0..10_000i64 {
        items.push(Item {
            id: i,
            name: format!("item_{}", i),
            value: (i as f64) * 3.14159,
            tags: vec![format!("tag_{}", i % 10), format!("tag_{}", i % 5)],
            nested: Nested { x: i, y: i * 2 },
        });
    }
    let blob = serde_json::to_string(&items).expect("stringify blob");

    // Warmup
    for _ in 0..3 {
        let parsed: Vec<Item> = serde_json::from_str(&blob).expect("parse");
        let _ = serde_json::to_string(&parsed).expect("stringify");
    }

    const ITERATIONS: usize = 50;
    let start = Instant::now();

    let mut checksum: usize = 0;
    for _ in 0..ITERATIONS {
        let parsed: Vec<Item> = serde_json::from_str(&blob).expect("parse");
        checksum += parsed.len();
        let re_stringified = serde_json::to_string(&parsed).expect("stringify");
        checksum += re_stringified.len();
    }

    let elapsed = start.elapsed().as_millis();
    println!("ms:{}", elapsed);
    println!("checksum:{}", checksum);
}
