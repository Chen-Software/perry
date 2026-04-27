// JSON parse-and-iterate polyglot benchmark — C++ (simdjson 4.x).
// 10k records, ~1 MB blob, 50 iterations.
// Per iteration: parse → sum every record's nested.x → stringify.
// IDENTICAL workload to bench_field_access.{ts,go,rs,swift,cpp,kt}.
//
// simdjson::ondemand is purpose-built for this shape: parse lazily,
// iterate exactly the fields you need, never materialize the full
// tree. This benchmark is where simdjson's design shines — every
// iteration is a stream of byte-level lookups against a SIMD-accelerated
// validator.
//
// Build flags: clang++ -std=c++17 -O3 -lsimdjson

#include <chrono>
#include <iostream>
#include <string>
#include <string_view>
#include <simdjson.h>

using namespace simdjson;

int main() {
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
    padded_string padded(blob);

    // Warmup
    for (int i = 0; i < 3; ++i) {
        ondemand::document doc = parser.iterate(padded);
        long long warm_sum = 0;
        for (auto element : doc.get_array()) {
            warm_sum += static_cast<long long>(int64_t(element["nested"]["x"]));
        }
        std::string_view raw = doc.raw_json();
        (void)raw;
        (void)warm_sum;
    }

    constexpr int iterations = 50;
    auto start = std::chrono::steady_clock::now();

    long long checksum = 0;
    for (int iter = 0; iter < iterations; ++iter) {
        ondemand::document doc = parser.iterate(padded);
        long long sum = 0;
        for (auto element : doc.get_array()) {
            sum += static_cast<long long>(int64_t(element["nested"]["x"]));
        }
        checksum += sum;
        std::string_view raw = doc.raw_json();
        std::string reStringified(raw);
        checksum += static_cast<long long>(reStringified.size());
    }

    auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now() - start).count();
    std::cout << "ms:" << elapsed << "\n";
    std::cout << "checksum:" << checksum << "\n";
    return 0;
}
