// demonstrates: threading + native-UI integration patterns shown in
//   docs/src/threading/overview.md and docs/src/threading/spawn.md
// docs: docs/src/threading/overview.md, docs/src/threading/spawn.md
// platforms: macos, linux

// The two ANCHOR blocks below are the snippets the threading docs render
// inline (via {{#include ... :NAME}}). They each show how `spawn` keeps a
// native UI responsive while heavy work runs on a background thread.
//
// The doc snippets in the .md file are deliberately fragment-y (a Button
// declaration followed by a sibling Text widget) to keep prose flow tight.
// We anchor exactly that shape here. Around the anchor we wire the widgets
// into an App({...}) so the file actually launches under the doc-tests
// harness's PERRY_UI_TEST_MODE and exits cleanly.

import { App, VStack, Text, Button, State } from "perry/ui"
import { spawn } from "perry/thread"

// Reactive state used by both snippets.
const status = State("Ready")
const result = State("")

// ANCHOR: ui-keep-responsive
const responsiveButton = Button("Start Analysis", async () => {
    status.set("Analyzing...")

    // Heavy computation runs on a background thread
    // UI stays responsive — user can still interact
    const value = await spawn(() => {
        let acc = 0
        for (let i = 0; i < 1_000_000; i++) acc += i
        return acc
    })

    status.set(`Done: ${value}`)
})

const responsiveText = Text(`Status: ${status.value}`)
// ANCHOR_END: ui-keep-responsive

// ANCHOR: ui-spawn-analyze
const analyzeButton = Button("Analyze", async () => {
    status.set("Processing...")

    // Background thread — UI stays responsive
    const data = await spawn(() => {
        let count = 0
        for (let i = 0; i < 1_000_000; i++) {
            if ((i & 0xff) === 0) count++
        }
        return { count }
    })

    result.set(`Found ${data.count} patterns`)
    status.set("Done")
})
// ANCHOR_END: ui-spawn-analyze

App({
    title: "Threading + UI",
    width: 400,
    height: 240,
    body: VStack(10, [
        Text(`Status: ${status.value}`),
        Text(`Result: ${result.value}`),
        responsiveButton,
        responsiveText,
        analyzeButton,
    ]),
})
