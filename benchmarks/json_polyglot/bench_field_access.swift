// JSON parse-and-iterate polyglot benchmark — Swift (Foundation).
// 10k records, ~1 MB blob, 50 iterations.
// Per iteration: parse → sum every record's nested.x → stringify.
// Identical workload to bench_field_access.ts/.go/.rs/.cpp/.kt.

import Foundation

struct FANested: Codable {
    let x: Int
    let y: Int
}

struct FAItem: Codable {
    let id: Int
    let name: String
    let value: Double
    let tags: [String]
    let nested: FANested
}

let encoder = JSONEncoder()
let decoder = JSONDecoder()

var items: [FAItem] = []
items.reserveCapacity(10_000)
for i in 0..<10_000 {
    items.append(FAItem(
        id: i,
        name: "item_\(i)",
        value: Double(i) * 3.14159,
        tags: ["tag_\(i % 10)", "tag_\(i % 5)"],
        nested: FANested(x: i, y: i * 2)
    ))
}
let blob = try encoder.encode(items)

// Warmup
for _ in 0..<3 {
    let parsed = try decoder.decode([FAItem].self, from: blob)
    var warmSum = 0
    for item in parsed {
        warmSum += item.nested.x
    }
    _ = try encoder.encode(parsed)
    _ = warmSum
}

let iterations = 50
let start = DispatchTime.now()

var checksum = 0
for _ in 0..<iterations {
    let parsed = try decoder.decode([FAItem].self, from: blob)
    var sum = 0
    for item in parsed {
        sum += item.nested.x
    }
    checksum += sum
    let reStringified = try encoder.encode(parsed)
    checksum += reStringified.count
}

let elapsedNs = DispatchTime.now().uptimeNanoseconds - start.uptimeNanoseconds
let elapsedMs = Int(elapsedNs / 1_000_000)
print("ms:\(elapsedMs)")
print("checksum:\(checksum)")
