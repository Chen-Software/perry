// demonstrates: perry/thread primitives (parallelMap, parallelFilter, spawn)
// docs: docs/src/threading/overview.md
// platforms: macos, linux
//
// Regression test for #146: before v0.5.167 the NATIVE_MODULE_TABLE in
// perry-codegen had no entries for perry/thread, so all three calls fell
// through to the receiver-less early-out and silently returned undefined.
// This file runs every call and prints deterministic results so the harness
// diffs byte-for-byte against _expected/runtime/thread_primitives.stdout.
// Any regression that drops the table rows prints `undefined` on at least
// one line and the CI diff fails.

import { parallelMap, parallelFilter, spawn } from "perry/thread";

const nums = [1, 2, 3, 4, 5, 6, 7, 8];

// parallelMap: preserves order, transforms every element.
const doubled = parallelMap(nums, (x: number) => x * 2);
console.log("map_len:" + doubled.length);
console.log("map_first:" + doubled[0]);
console.log("map_last:" + doubled[7]);

// parallelFilter: keeps elements where the predicate returns truthy,
// order-preserving across chunks.
const evens = parallelFilter(nums, (x: number) => x % 2 === 0);
console.log("filter_len:" + evens.length);
console.log("filter_0:" + evens[0]);
console.log("filter_3:" + evens[3]);

// spawn: returns a Promise that resolves to the closure's return value
// on the main thread.
async function main(): Promise<void> {
    const result = await spawn(() => {
        let sum = 0;
        for (let i = 1; i <= 100; i = i + 1) {
            sum = sum + i;
        }
        return sum;
    });
    console.log("spawn_result:" + result);
}

main();
