// demonstrates: parallelMap + spawn shown in hello-world.md
// docs: docs/src/getting-started/hello-world.md
// platforms: macos, linux, windows

import { parallelMap, parallelFilter, spawn } from "perry/thread"

const data = [1, 2, 3, 4, 5, 6, 7, 8]

// Process all elements across all CPU cores
const doubled = parallelMap(data, (x: number) => x * 2)
console.log(doubled) // [2, 4, 6, 8, 10, 12, 14, 16]

// Run heavy work in the background
const result = await spawn(() => {
    let sum = 0
    for (let i = 0; i < 100_000_000; i++) sum += i
    return sum
})
console.log(result)

// parallelFilter is also available for the lift-and-parallelize case:
const evens = parallelFilter(data, (x: number) => x % 2 === 0)
console.log(evens)
