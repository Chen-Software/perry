// JSON parse-and-iterate polyglot benchmark — Rust (serde_json).
// 10k records, ~1 MB blob, 50 iterations.
// Per iteration: parse → sum every record's nested.x → stringify.
// Identical workload to bench_field_access.ts/.go/.swift/.cpp/.kt.

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
        let mut warm_sum: i64 = 0;
        for item in &parsed {
            warm_sum += item.nested.x;
        }
        let _ = serde_json::to_string(&parsed).expect("stringify");
        let _ = warm_sum;
    }

    const ITERATIONS: usize = 50;
    let start = Instant::now();

    let mut checksum: i64 = 0;
    for _ in 0..ITERATIONS {
        let parsed: Vec<Item> = serde_json::from_str(&blob).expect("parse");
        let mut sum: i64 = 0;
        for item in &parsed {
            sum += item.nested.x;
        }
        checksum += sum;
        let re_stringified = serde_json::to_string(&parsed).expect("stringify");
        checksum += re_stringified.len() as i64;
    }

    let elapsed = start.elapsed().as_millis();
    println!("ms:{}", elapsed);
    println!("checksum:{}", checksum);
}
