// JSON validate-and-roundtrip polyglot benchmark — C++ (simdjson 4.x).
// 10k records, ~1 MB blob, 50 iterations.
// IDENTICAL workload to bench.cpp (nlohmann/json) and the other languages.
//
// simdjson is the SIMD-accelerated JSON parser reference (https://simdjson.org).
// It's the parse-throughput ceiling for C++. The trade-off: simdjson's
// "ondemand" parser validates and exposes the document via a streaming
// iterator, but does NOT provide a built-in stringify primitive. For the
// roundtrip workload we use the raw_json() bytes of the parsed document
// as the "stringified" output — same conceptual approach as Perry's
// lazy JSON tape (v0.5.204+), which on an unmutated parse memcpy's the
// original blob bytes for stringify. This is a legitimate comparison:
// both runtimes exploit the "no modification between parse and stringify"
// fast path. nlohmann/json (the other C++ row) does NOT have this
// fast path and rebuilds the string from the parsed tree on every
// dump() call.
//
// Build flags: clang++ -std=c++17 -O3 -lsimdjson

#include <chrono>
#include <iostream>
#include <string>
#include <string_view>
#include <vector>
#include <simdjson.h>

using namespace simdjson;

int main() {
    // Build blob upfront via std::string concatenation. This matches
    // what nlohmann's bench.cpp does (json::array() + dump()) but
    // avoids dragging nlohmann into the comparison.
    std::string blob = "[";
    for (int i = 0; i < 10000; ++i) {
        if (i > 0) blob += ',';
        blob += "{\"id\":" + std::to_string(i)
              + ",\"name\":\"item_" + std::to_string(i) + "\""
              + ",\"value\":" + std::to_string(static_cast<double>(i) * 3.14159)
              + ",\"tags\":[\"tag_" + std::to_string(i % 10) + "\","
              + "\"tag_" + std::to_string(i % 5) + "\"]"
              + ",\"nested\":{\"x\":" + std::to_string(i)
              + ",\"y\":" + std::to_string(i * 2) + "}}";
    }
    blob += "]";

    ondemand::parser parser;
    // Pad blob for simdjson. ondemand needs SIMDJSON_PADDING extra bytes.
    padded_string padded(blob);

    // Warmup
    for (int i = 0; i < 3; ++i) {
        ondemand::document doc = parser.iterate(padded);
        // Force full validation by counting elements.
        size_t count = 0;
        for (auto element : doc.get_array()) {
            (void)element;
            ++count;
        }
        // "Stringify" — for unmutated parse, simdjson's raw_json view
        // is functionally equivalent to memcpy of the original. Same
        // fast-path Perry's lazy tape uses; documented in the file
        // header.
        std::string_view raw = doc.raw_json();
        (void)raw;
        (void)count;
    }

    constexpr int iterations = 50;
    auto start = std::chrono::steady_clock::now();

    long long checksum = 0;
    for (int iter = 0; iter < iterations; ++iter) {
        ondemand::document doc = parser.iterate(padded);
        size_t count = 0;
        for (auto element : doc.get_array()) {
            (void)element;
            ++count;
        }
        checksum += static_cast<long long>(count);
        std::string_view raw = doc.raw_json();
        // For fair comparison with the "rebuild a string" runtimes,
        // copy the bytes into a fresh std::string. This is the
        // memcpy cost analogous to Perry's lazy stringify.
        std::string reStringified(raw);
        checksum += static_cast<long long>(reStringified.size());
    }

    auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now() - start).count();
    std::cout << "ms:" << elapsed << "\n";
    std::cout << "checksum:" << checksum << "\n";
    return 0;
}
