// JSON parse-and-iterate polyglot benchmark — Kotlin (kotlinx.serialization).
// 10k records, ~1 MB blob, 50 iterations.
// Per iteration: parse → sum every record's nested.x → stringify.
// Identical workload to bench_field_access.ts/.go/.rs/.swift/.cpp.

import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.json.Json

@Serializable
data class FANested(val x: Int, val y: Int)

@Serializable
data class FAItem(
    val id: Int,
    val name: String,
    val value: Double,
    val tags: List<String>,
    val nested: FANested,
)

fun main() {
    val items = (0 until 10_000).map { i ->
        FAItem(
            id = i,
            name = "item_$i",
            value = i * 3.14159,
            tags = listOf("tag_${i % 10}", "tag_${i % 5}"),
            nested = FANested(x = i, y = i * 2),
        )
    }
    val blob: String = Json.encodeToString(items)

    // Warmup
    repeat(3) {
        val parsed = Json.decodeFromString<List<FAItem>>(blob)
        var warmSum = 0L
        for (item in parsed) {
            warmSum += item.nested.x.toLong()
        }
        Json.encodeToString(parsed)
    }

    val iterations = 50
    val start = System.currentTimeMillis()

    var checksum = 0L
    repeat(iterations) {
        val parsed = Json.decodeFromString<List<FAItem>>(blob)
        var sum = 0L
        for (item in parsed) {
            sum += item.nested.x.toLong()
        }
        checksum += sum
        val reStringified = Json.encodeToString(parsed)
        checksum += reStringified.length.toLong()
    }

    val elapsed = System.currentTimeMillis() - start
    println("ms:$elapsed")
    println("checksum:$checksum")
}
