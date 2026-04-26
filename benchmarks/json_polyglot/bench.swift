// JSON parse + stringify polyglot benchmark — Swift (Foundation JSONEncoder/JSONDecoder).
// 10k records, ~1 MB blob, 50 iterations.
// IDENTICAL workload to bench.ts / bench.go / bench.rs / bench.cpp / bench.js.

import Foundation

struct Nested: Codable {
    let x: Int
    let y: Int
}

struct Item: Codable {
    let id: Int
    let name: String
    let value: Double
    let tags: [String]
    let nested: Nested
}

let encoder = JSONEncoder()
let decoder = JSONDecoder()

var items: [Item] = []
items.reserveCapacity(10_000)
for i in 0..<10_000 {
    items.append(Item(
        id: i,
        name: "item_\(i)",
        value: Double(i) * 3.14159,
        tags: ["tag_\(i % 10)", "tag_\(i % 5)"],
        nested: Nested(x: i, y: i * 2)
    ))
}
let blob = try encoder.encode(items)

// Warmup
for _ in 0..<3 {
    let parsed = try decoder.decode([Item].self, from: blob)
    _ = try encoder.encode(parsed)
}

let iterations = 50
let start = DispatchTime.now()

var checksum = 0
for _ in 0..<iterations {
    let parsed = try decoder.decode([Item].self, from: blob)
    checksum += parsed.count
    let reStringified = try encoder.encode(parsed)
    checksum += reStringified.count
}

let elapsedNs = DispatchTime.now().uptimeNanoseconds - start.uptimeNanoseconds
let elapsedMs = Int(elapsedNs / 1_000_000)
print("ms:\(elapsedMs)")
print("checksum:\(checksum)")
