// demonstrates: compile-time __platform__ constant for platform branching
// docs: docs/src/platforms/overview.md, tvos.md, visionos.md, watchos.md
// platforms: macos, linux, windows

// Each ANCHOR block below is the exact code that the platform docs render
// inline (via {{#include ... :NAME}}). The whole file is compiled and run
// by the doc-tests harness, so every snippet is a tested artifact — if any
// snippet drifts from the real `__platform__` plumbing, CI fails.
//
// `__platform__` is resolved at compile time by perry-codegen — it's a
// `declare const`-style constant the compiler folds into the call site, so
// platform-specific branches have zero runtime cost.

// ANCHOR: overview-detect
declare const __platform__: number

// Platform constants:
// 0 = macOS
// 1 = iOS
// 2 = Android
// 3 = Windows
// 4 = Linux
// 5 = Web (browser, --target web / --target wasm)
// 6 = tvOS
// 7 = watchOS
// 8 = visionOS

if (__platform__ === 0) {
    console.log("Running on macOS")
} else if (__platform__ === 1) {
    console.log("Running on iOS")
} else if (__platform__ === 3) {
    console.log("Running on Windows")
}
// ANCHOR_END: overview-detect

// ANCHOR: tvos-detect
function reportTvos(): void {
    if (__platform__ === 6) {
        console.log("Running on tvOS")
    }
}
// ANCHOR_END: tvos-detect

// ANCHOR: visionos-detect
function reportVisionos(): void {
    if (__platform__ === 8) {
        console.log("Running on visionOS")
    }
}
// ANCHOR_END: visionos-detect

// ANCHOR: watchos-detect
function reportWatchos(): void {
    if (__platform__ === 7) {
        console.log("Running on watchOS")
    }
}
// ANCHOR_END: watchos-detect

reportTvos()
reportVisionos()
reportWatchos()
