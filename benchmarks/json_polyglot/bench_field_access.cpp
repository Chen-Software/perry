// JSON parse-and-iterate polyglot benchmark — C++ (nlohmann/json).
// 10k records, ~1 MB blob, 50 iterations.
// Per iteration: parse → sum every record's nested.x → stringify.
// Identical workload to bench_field_access.ts/.go/.rs/.swift/.kt.

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
        long long warm_sum = 0;
        for (const auto& item : parsed) {
            warm_sum += item["nested"]["x"].get<long long>();
        }
        (void)parsed.dump();
        (void)warm_sum;
    }

    constexpr int iterations = 50;
    auto start = std::chrono::steady_clock::now();

    long long checksum = 0;
    for (int iter = 0; iter < iterations; ++iter) {
        json parsed = json::parse(blob);
        long long sum = 0;
        for (const auto& item : parsed) {
            sum += item["nested"]["x"].get<long long>();
        }
        checksum += sum;
        std::string reStringified = parsed.dump();
        checksum += static_cast<long long>(reStringified.size());
    }

    auto elapsed = std::chrono::duration_cast<std::chrono::milliseconds>(
        std::chrono::steady_clock::now() - start).count();
    std::cout << "ms:" << elapsed << "\n";
    std::cout << "checksum:" << checksum << "\n";
    return 0;
}
