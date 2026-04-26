// demonstrates: per-API thread snippets shown in docs/src/threading/*.md
// docs: docs/src/threading/spawn.md, parallel-map.md, parallel-filter.md, overview.md
// platforms: macos, linux

// Each ANCHOR block below is the exact code that the threading docs render
// inline (via {{#include ... :NAME}}). The whole file is compiled and run by
// the doc-tests harness, so every snippet is a tested artifact — if any
// snippet drifts from the real perry/thread API, CI fails.
//
// Where the docs show fictional helpers like `analyzeDataset(...)` or
// `heavyComputation(x)`, the snippet here uses inline arithmetic / loops so
// the file actually compiles and runs to a clean exit. The shape of the
// snippet (call to spawn / parallelMap / parallelFilter, capture pattern,
// return type) is preserved.

import { spawn, parallelMap, parallelFilter } from "perry/thread"

// -----------------------------------------------------------------------------
// overview.md
// -----------------------------------------------------------------------------

// ANCHOR: overview-header
async function overviewHeader(): Promise<void> {
    const data = [1, 2, 3, 4, 5, 6, 7, 8]
    const records = [
        { score: 50, id: 1 },
        { score: 90, id: 2 },
        { score: 85, id: 3 },
    ]
    const threshold = 80

    // Process a million items across all CPU cores
    const results = parallelMap(data, (item: number) => item * item)

    // Filter a large dataset in parallel
    const valid = parallelFilter(records, (r: { score: number; id: number }) => r.score > threshold)

    // Run expensive work in the background
    const answer = await spawn(() => {
        let acc = 0
        for (let i = 0; i < 100_000; i++) acc += i
        return acc
    })

    console.log(`overview-header results=${results.length} valid=${valid.length} answer=${answer}`)
}
// ANCHOR_END: overview-header

// ANCHOR: overview-parallel-map
async function overviewParallelMap(): Promise<void> {
    const prices = [100, 200, 300, 400, 500, 600, 700, 800]
    const adjusted = parallelMap(prices, (price: number) => {
        // Heavy computation runs on a worker thread
        let result = price
        for (let i = 0; i < 1000000; i++) {
            result = Math.sqrt(result * result + i)
        }
        return result
    })

    console.log(`overview-parallel-map len=${adjusted.length}`)
}
// ANCHOR_END: overview-parallel-map

// ANCHOR: overview-parallel-filter
async function overviewParallelFilter(): Promise<void> {
    const cutoffDate = 1_700_000_000
    const users = [
        { lastLogin: 1_710_000_000, score: 150, name: "alice" },
        { lastLogin: 1_690_000_000, score: 50, name: "bob" },
        { lastLogin: 1_720_000_000, score: 120, name: "carol" },
    ]

    // Filter across all cores — order is preserved
    const active = parallelFilter(users, (user: { lastLogin: number; score: number; name: string }) => {
        return user.lastLogin > cutoffDate && user.score > 100
    })

    console.log(`overview-parallel-filter active=${active.length}`)
}
// ANCHOR_END: overview-parallel-filter

// ANCHOR: overview-spawn-bg
async function overviewSpawnBg(): Promise<void> {
    // Start heavy work in the background
    const handle = spawn(() => {
        let sum = 0
        for (let i = 0; i < 100_000; i++) {
            sum += Math.sin(i)
        }
        return sum
    })

    // Main thread keeps running — UI stays responsive
    console.log("Computing...")

    // Get the result when you need it
    const result = await handle
    console.log(`Done: len=${typeof result}`)
}
// ANCHOR_END: overview-spawn-bg

// ANCHOR: overview-image
async function overviewImage(): Promise<void> {
    const pixels = [
        { r: 100, g: 120, b: 140 },
        { r: 50, g: 60, b: 70 },
        { r: 200, g: 210, b: 220 },
    ]

    // Each pixel processed on a separate core
    const processed = parallelMap(pixels, (pixel: { r: number; g: number; b: number }) => {
        const r = Math.min(255, pixel.r * 1.2)
        const g = Math.min(255, pixel.g * 0.8)
        const b = Math.min(255, pixel.b * 1.1)
        return { r, g, b }
    })

    console.log(`overview-image processed=${processed.length}`)
}
// ANCHOR_END: overview-image

// ANCHOR: overview-crypto
async function overviewCrypto(): Promise<void> {
    // Hash thousands of items across all cores
    const passwords = ["pass1", "pass2", "pass3"]
    const hashed = parallelMap(passwords, (password: string) => {
        // Stand-in for a real hash: deterministic FNV-1a over the bytes.
        let h = 2166136261
        for (let i = 0; i < password.length; i++) {
            h ^= password.charCodeAt(i)
            h = (h * 16777619) >>> 0
        }
        return h
    })

    console.log(`overview-crypto hashed=${hashed.length}`)
}
// ANCHOR_END: overview-crypto

// ANCHOR: overview-multiple
async function overviewMultiple(): Promise<void> {
    const dataA = [1, 2, 3]
    const dataB = [4, 5, 6]
    const dataC = [7, 8, 9]

    // Three independent tasks run simultaneously on three OS threads
    const task1 = spawn(() => {
        let acc = 0
        for (const v of dataA) acc += v * v
        return acc
    })
    const task2 = spawn(() => {
        let acc = 0
        for (const v of dataB) acc += v * v
        return acc
    })
    const task3 = spawn(() => {
        let acc = 0
        for (const v of dataC) acc += v * v
        return acc
    })

    // All three run concurrently
    const [result1, result2, result3] = await Promise.all([task1, task2, task3])
    console.log(`overview-multiple ${result1} ${result2} ${result3}`)
}
// ANCHOR_END: overview-multiple

// ANCHOR: overview-captured
async function overviewCaptured(): Promise<void> {
    const prices = [100, 200, 300, 400]
    const taxRate = 0.08
    const discount = 0.15

    // taxRate and discount are captured and copied to each thread
    const finalPrices = parallelMap(prices, (price: number) => {
        const discounted = price * (1 - discount)
        return discounted * (1 + taxRate)
    })

    console.log(`overview-captured len=${finalPrices.length}`)
}
// ANCHOR_END: overview-captured

// ANCHOR: overview-reduce-instead
async function overviewReduceInstead(): Promise<void> {
    const data = [1, 2, 3, 4, 5, 6, 7, 8]

    // Instead of mutating a shared counter, return values and reduce
    const results = parallelMap(data, (item: number) => item * item)
    const total = results.reduce((sum: number, r: number) => sum + r, 0)

    console.log(`overview-reduce-instead total=${total}`)
}
// ANCHOR_END: overview-reduce-instead

// -----------------------------------------------------------------------------
// spawn.md
// -----------------------------------------------------------------------------

// ANCHOR: spawn-basic
async function spawnBasic(): Promise<void> {
    const result = await spawn(() => {
        // This runs on a separate OS thread.
        let sum = 0
        for (let i = 0; i < 100_000_000; i++) {
            sum += i
        }
        return sum
    })

    console.log(result) // 4999999950000000
}
// ANCHOR_END: spawn-basic

// ANCHOR: spawn-non-blocking
async function spawnNonBlocking(): Promise<void> {
    console.log("1. Starting background work")

    const handle = spawn(() => {
        // Runs on a background thread — heavier work elided here.
        let n = 0
        for (let i = 0; i < 10_000_000; i++) n++
        return n
    })

    console.log("2. Main thread continues immediately")

    const result = await handle
    console.log(`3. Got result: ${result}`)
}
// ANCHOR_END: spawn-non-blocking

// ANCHOR: spawn-multiple
async function spawnMultiple(): Promise<void> {
    const t1 = spawn(() => analyseChunk(0, 1_000_000))
    const t2 = spawn(() => analyseChunk(1_000_000, 2_000_000))
    const t3 = spawn(() => analyseChunk(2_000_000, 3_000_000))

    // All three run simultaneously on separate OS threads.
    const results = await Promise.all([t1, t2, t3])

    console.log(`Region A: ${results[0]}`)
    console.log(`Region B: ${results[1]}`)
    console.log(`Region C: ${results[2]}`)
}

function analyseChunk(start: number, end: number): number {
    let acc = 0
    for (let i = start; i < end; i++) acc += i & 0xff
    return acc
}
// ANCHOR_END: spawn-multiple

// ANCHOR: spawn-capture
async function spawnCapture(): Promise<void> {
    const config = { iterations: 1000, seed: 42 }
    const dataset = [1, 2, 3, 4, 5, 6, 7, 8]

    const result = await spawn(() => {
        // config and dataset are deep-copied to this thread.
        let acc = config.seed
        for (let i = 0; i < config.iterations; i++) {
            acc = (acc * 1103515245 + 12345) & 0x7fffffff
        }
        for (const v of dataset) acc ^= v
        return acc
    })

    console.log(`spawn-capture: ${result}`)
}
// ANCHOR_END: spawn-capture

// ANCHOR: spawn-complex-return
async function spawnComplexReturn(): Promise<void> {
    const stats = await spawn(() => {
        const values = [3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0]
        let sum = 0
        let max = values[0]
        let min = values[0]
        for (const v of values) {
            sum += v
            if (v > max) max = v
            if (v < min) min = v
        }
        return {
            mean: sum / values.length,
            min,
            max,
            count: values.length,
        }
    })

    console.log(`mean=${stats.mean} min=${stats.min} max=${stats.max} count=${stats.count}`)
}
// ANCHOR_END: spawn-complex-return

// ANCHOR: spawn-bg-file
async function spawnBgFile(): Promise<void> {
    // Read and process a "large" file without blocking. We inline a tiny CSV
    // so the snippet runs hermetically — the docs' real version would call
    // readFileSync from "fs".
    const content = "id,value\n1,10\n2,20\n3,30\n"
    const analysis = await spawn(() => {
        const lines = content.split("\n").filter((l: string) => l.length > 0).slice(1)
        let total = 0
        for (const line of lines) {
            const parts = line.split(",")
            total += parseInt(parts[1], 10)
        }
        return { rows: lines.length, total }
    })

    console.log(`spawn-bg-file rows=${analysis.rows} total=${analysis.total}`)
}
// ANCHOR_END: spawn-bg-file

// ANCHOR: spawn-api-then-process
async function spawnApiThenProcess(): Promise<void> {
    // The docs example fetches a remote API; for a hermetic test we
    // just hand-roll the same pipeline shape with synthetic data.
    const rawData = { items: [1, 2, 3, 4, 5] }

    // CPU-intensive processing happens off the main thread
    const processed = await spawn(() => {
        let total = 0
        for (const v of rawData.items) total += v * v
        return { total, count: rawData.items.length }
    })

    console.log(`spawn-api-then-process total=${processed.total} count=${processed.count}`)
}
// ANCHOR_END: spawn-api-then-process

// ANCHOR: spawn-deferred
async function spawnDeferred(): Promise<void> {
    const params = { size: 8 }

    // Start computation early, use result later
    const precomputed = spawn(() => {
        const table: number[] = []
        for (let i = 0; i < params.size; i++) table.push(i * i)
        return table
    })

    // ... do other setup work ...

    // Result is ready (or we wait for it)
    const table = await precomputed
    console.log(`spawn-deferred len=${table.length} last=${table[table.length - 1]}`)
}
// ANCHOR_END: spawn-deferred

// -----------------------------------------------------------------------------
// parallel-map.md
// -----------------------------------------------------------------------------

// ANCHOR: parallel-map
const nums = [1, 2, 3, 4, 5, 6, 7, 8]

// parallelMap preserves order; the closure runs on a worker thread.
const doubled = parallelMap(nums, (x: number) => x * 2)
console.log(`map_len=${doubled.length} first=${doubled[0]} last=${doubled[7]}`)
// ANCHOR_END: parallel-map

// ANCHOR: parallel-map-basic
function parallelMapBasic(): void {
    const numbers = [1, 2, 3, 4, 5, 6, 7, 8]
    const doubled = parallelMap(numbers, (x: number) => x * 2)
    // [2, 4, 6, 8, 10, 12, 14, 16]
    console.log(`parallel-map-basic len=${doubled.length}`)
}
// ANCHOR_END: parallel-map-basic

// ANCHOR: parallel-map-capture
function parallelMapCapture(): void {
    const prices = [100, 200, 300]
    const exchangeRate = 1.12

    const converted = parallelMap(prices, (price: number) => {
        // exchangeRate is captured and copied to each thread
        return price * exchangeRate
    })

    console.log(`parallel-map-capture len=${converted.length}`)
}
// ANCHOR_END: parallel-map-capture

// ANCHOR: parallel-map-reduce
function parallelMapReduce(): void {
    const data = [1, 2, 3, 4, 5, 6, 7, 8]
    const results = parallelMap(data, (item: number) => item * 2)
    const total = results.reduce((sum: number, x: number) => sum + x, 0)
    console.log(`parallel-map-reduce total=${total}`)
}
// ANCHOR_END: parallel-map-reduce

// ANCHOR: parallel-map-good-candidates
function parallelMapGoodCandidates(): void {
    const data = [1.0, 2.0, 3.0, 4.0]
    const documents = ["alpha beta", "gamma delta", "epsilon"]
    const inputs = ["a", "bb", "ccc"]

    // Heavy math
    const out1 = parallelMap(data, (x: number) => {
        let acc = x
        for (let i = 0; i < 1_000; i++) acc = Math.sqrt(acc * acc + i)
        return acc
    })

    // String processing on large strings
    const out2 = parallelMap(documents, (doc: string) => {
        const words = doc.split(" ")
        return { count: words.length, first: words[0] }
    })

    // Cryptographic operations
    const out3 = parallelMap(inputs, (input: string) => {
        let h = 0
        for (let i = 0; i < input.length; i++) h = (h * 31 + input.charCodeAt(i)) >>> 0
        return h
    })

    console.log(`parallel-map-good-candidates ${out1.length} ${out2.length} ${out3.length}`)
}
// ANCHOR_END: parallel-map-good-candidates

// ANCHOR: parallel-map-poor-candidate
function parallelMapPoorCandidate(): void {
    const numbers = [1, 2, 3, 4, 5]

    // Too simple — threading overhead outweighs the gain
    const a = parallelMap(numbers, (x: number) => x + 1)

    // For trivial operations, use regular map
    const result = numbers.map((x: number) => x + 1)

    console.log(`parallel-map-poor-candidate ${a.length} ${result.length}`)
}
// ANCHOR_END: parallel-map-poor-candidate

// ANCHOR: parallel-map-matrix
function parallelMapMatrix(): void {
    // Process each row of a matrix independently
    const rows = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]
    const rowSums = parallelMap(rows, (row: number[]) => {
        let sum = 0
        for (const val of row) sum += val
        return sum
    })
    // [6, 15, 24]
    console.log(`parallel-map-matrix sums=${rowSums[0]},${rowSums[1]},${rowSums[2]}`)
}
// ANCHOR_END: parallel-map-matrix

// ANCHOR: parallel-map-validation
function parallelMapValidation(): void {
    const users = [
        { name: "Alice", email: "alice@example.com" },
        { name: "Bob", email: "invalid" },
        { name: "Charlie", email: "charlie@example.com" },
    ]

    const validationResults = parallelMap(users, (user: { name: string; email: string }) => {
        const emailValid = user.email.includes("@") && user.email.includes(".")
        const nameValid = user.name.length > 0 && user.name.length < 100
        return { name: user.name, valid: emailValid && nameValid }
    })

    console.log(`parallel-map-validation len=${validationResults.length}`)
}
// ANCHOR_END: parallel-map-validation

// ANCHOR: parallel-map-monte-carlo
function parallelMapMonteCarlo(): void {
    const portfolios = [
        { id: 1, base: 100 },
        { id: 2, base: 200 },
        { id: 3, base: 150 },
    ] // thousands of portfolios

    // Monte Carlo simulation across all cores
    const riskScores = parallelMap(portfolios, (portfolio: { id: number; base: number }) => {
        let totalRisk = 0
        for (let sim = 0; sim < 1000; sim++) {
            // simulateReturns stand-in: deterministic pseudo-random walk.
            let s = portfolio.base + sim
            s = ((s * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff
            totalRisk += s
        }
        return totalRisk / 1000
    })

    console.log(`parallel-map-monte-carlo len=${riskScores.length}`)
}
// ANCHOR_END: parallel-map-monte-carlo

// -----------------------------------------------------------------------------
// parallel-filter.md
// -----------------------------------------------------------------------------

// ANCHOR: parallel-filter
const evens = parallelFilter(nums, (x: number) => x % 2 === 0)
console.log(`filter_len=${evens.length} first=${evens[0]} last=${evens[evens.length - 1]}`)
// ANCHOR_END: parallel-filter

// ANCHOR: parallel-filter-basic
function parallelFilterBasic(): void {
    const numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    const evens = parallelFilter(numbers, (x: number) => x % 2 === 0)
    // [2, 4, 6, 8, 10]
    console.log(`parallel-filter-basic len=${evens.length}`)
}
// ANCHOR_END: parallel-filter-basic

// ANCHOR: parallel-filter-vs-filter
function parallelFilterVsFilter(): void {
    const data = [1, 2, 3, 4, 5, 6, 7, 8]

    // Single-threaded — one core does all the work
    const a = data.filter((item: number) => item > 3)

    // Parallel — all cores share the work
    const b = parallelFilter(data, (item: number) => item > 3)

    console.log(`parallel-filter-vs-filter ${a.length} ${b.length}`)
}
// ANCHOR_END: parallel-filter-vs-filter

// ANCHOR: parallel-filter-capture
function parallelFilterCapture(): void {
    const candidates = [
        { name: "Alice", score: 90, age: 28 },
        { name: "Bob", score: 80, age: 35 },
        { name: "Carol", score: 95, age: 25 },
    ]
    const minScore = 85
    const maxAge = 30

    // minScore and maxAge are captured and copied to each thread
    const qualified = parallelFilter(candidates, (c: { name: string; score: number; age: number }) => {
        return c.score >= minScore && c.age <= maxAge
    })

    console.log(`parallel-filter-capture len=${qualified.length}`)
}
// ANCHOR_END: parallel-filter-capture

// ANCHOR: parallel-filter-large
function parallelFilterLarge(): void {
    // Stand-in for "millions of records" — same shape, smaller list.
    const transactions = [
        { amount: 15000, country: "US", user: { homeCountry: "DE" }, timestamp: { hour: 4 } },
        { amount: 200, country: "DE", user: { homeCountry: "DE" }, timestamp: { hour: 12 } },
        { amount: 50000, country: "FR", user: { homeCountry: "DE" }, timestamp: { hour: 3 } },
    ]

    const suspicious = parallelFilter(transactions, (tx: {
        amount: number
        country: string
        user: { homeCountry: string }
        timestamp: { hour: number }
    }) => {
        return tx.amount > 10000
            && tx.country !== tx.user.homeCountry
            && tx.timestamp.hour < 6
    })

    console.log(`parallel-filter-large len=${suspicious.length}`)
}
// ANCHOR_END: parallel-filter-large

// ANCHOR: parallel-filter-combined
function parallelFilterCombined(): void {
    const users = [
        { name: "Alice", isActive: true, age: 28, score: 90 },
        { name: "Bob", isActive: false, age: 35, score: 80 },
        { name: "Carol", isActive: true, age: 17, score: 95 },
        { name: "Dave", isActive: true, age: 40, score: 60 },
    ]

    // Step 1: Filter to relevant items (parallel)
    const active = parallelFilter(users, (u: { isActive: boolean; age: number }) => u.isActive && u.age >= 18)

    // Step 2: Transform the filtered results (parallel)
    const profiles = parallelMap(active, (u: { name: string; score: number }) => ({
        name: u.name,
        score: u.score * 2,
    }))

    console.log(`parallel-filter-combined active=${active.length} profiles=${profiles.length}`)
}
// ANCHOR_END: parallel-filter-combined

// ANCHOR: parallel-filter-heavy
function parallelFilterHeavy(): void {
    const certificates = [
        { id: 1, fingerprint: "aa", revoked: false },
        { id: 2, fingerprint: "bb", revoked: true },
        { id: 3, fingerprint: "cc", revoked: false },
    ]

    // Each predicate call does significant work — perfect for parallelization.
    const valid = parallelFilter(certificates, (cert: { id: number; fingerprint: string; revoked: boolean }) => {
        // Stand-in for a real chain verification: hash the fingerprint a bit
        // then sanity-check the revocation flag.
        let h = 0
        for (let i = 0; i < cert.fingerprint.length; i++) {
            h = (h * 31 + cert.fingerprint.charCodeAt(i)) >>> 0
        }
        return h !== 0 && !cert.revoked
    })

    console.log(`parallel-filter-heavy len=${valid.length}`)
}
// ANCHOR_END: parallel-filter-heavy

// -----------------------------------------------------------------------------
// driver
// -----------------------------------------------------------------------------

async function main(): Promise<void> {
    await overviewHeader()
    await overviewParallelMap()
    await overviewParallelFilter()
    await overviewSpawnBg()
    await overviewImage()
    await overviewCrypto()
    await overviewMultiple()
    await overviewCaptured()
    await overviewReduceInstead()

    await spawnBasic()
    await spawnNonBlocking()
    await spawnMultiple()
    await spawnCapture()
    await spawnComplexReturn()
    await spawnBgFile()
    await spawnApiThenProcess()
    await spawnDeferred()

    parallelMapBasic()
    parallelMapCapture()
    parallelMapReduce()
    parallelMapGoodCandidates()
    parallelMapPoorCandidate()
    parallelMapMatrix()
    parallelMapValidation()
    parallelMapMonteCarlo()

    parallelFilterBasic()
    parallelFilterVsFilter()
    parallelFilterCapture()
    parallelFilterLarge()
    parallelFilterCombined()
    parallelFilterHeavy()
}

main()
