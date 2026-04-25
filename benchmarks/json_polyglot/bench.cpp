// JSON parse + stringify polyglot benchmark — C++ (nlohmann/json).
// 10k records, ~1 MB blob, 50 iterations.
// IDENTICAL workload to bench.ts / bench.go / bench.rs / bench.swift / bench.js.
//
// nlohmann/json is the de facto standard JSON library for C++. It's not
// the fastest (RapidJSON / simdjson are faster) but it's the most idiomatic
// and what most C++ projects reach for. Aggressive flag set in run.sh
// could swap to a faster library if we want to show the SIMD ceiling.

#include <chrono>
#include <iostream>
#include <string>
#include <vector>
#include <nlohmann/json.hpp>

using json = nlohmann::json;

int main() {
    json items = json::array();
    for (int i = 0; i < 10000; ++i) {
        json tags = json::array();
        tags.push_back("tag_" + std::to_string(i % 10));
        tags.push_back("tag_" + std::to_string(i % 5));
        items.push_back({
            {"id", i},
            {"name", "item_" + std::to_string(i)},
            {"value", static_cast<double>(i) * 3.14159},
            {"tags", tags},
            {"nested", { {"x", i}, {"y", i * 2} }}
        });
    }
    std::string blob = items.dump();

    // Warmup
    for (int i = 0; i < 3; ++i) {
        json parsed = json::parse(blob);
        (void)parsed.dump();
    }

    constexpr int iterations = 50;
    auto start = std::chrono::steady_clock::now();

    long long checksum = 0;
    for (int iter = 0; iter < iterations; ++iter) {
        json parsed = json::parse(blob);
        checksum += static_cast<long long>(parsed.size());
        std::string reStringified = parsed.dump();
        checksum += static_cast<long long>(reStringified.size());
    }

    auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now() - start).count();
    std::cout << "ms:" << elapsed << "\n";
    std::cout << "checksum:" << checksum << "\n";
    return 0;
}
