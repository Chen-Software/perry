// demonstrates: per-API getting-started snippets (hello-world.md, project-config.md, introduction.md)
// docs: docs/src/getting-started/hello-world.md, project-config.md, introduction.md
// platforms: macos, linux, windows

// Each ANCHOR block below is the exact code that the getting-started docs
// render inline (via {{#include ... :NAME}}). The whole file is compiled
// and run by the doc-tests harness, so every snippet is a tested artifact —
// if anything drifts from the real perry/* API, CI fails.

// ANCHOR: variables-functions
const name: string = "World"
const items: number[] = [1, 2, 3, 4, 5]

const doubled = items.map((x) => x * 2)
const sum = doubled.reduce((acc, x) => acc + x, 0)

console.log(`Hello, ${name}!`)
console.log(`Sum of doubled: ${sum}`)
// ANCHOR_END: variables-functions
