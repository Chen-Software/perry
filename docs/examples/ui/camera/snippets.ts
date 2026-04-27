// demonstrates: per-API camera snippets shown in docs/src/ui/camera.md
// docs: docs/src/ui/camera.md
// platforms: macos, linux, windows
// run: false

// `run: false` because the live camera FFI is only meaningful on iOS and
// Android — on macOS / Linux / Windows the runtime exports no-op stubs,
// and even on the real platforms the camera dialog and capture session
// can't run inside the doc-tests harness. Compile-link is enough to
// certify the codegen surface; this file pins every name in the camera
// API down so a future rename / drop trips a link error in CI.

// ANCHOR: imports
import {
    CameraView,
    cameraStart, cameraStop,
    cameraFreeze, cameraUnfreeze,
    cameraSampleColor, cameraSetOnTap,
} from "perry/ui"
// ANCHOR_END: imports
import { App, VStack, Text, State } from "perry/ui"

// ANCHOR: quick-example
const colorHex = State("#000000")

const cam = CameraView()
cameraStart(cam)

cameraSetOnTap(cam, (x: number, y: number) => {
    const rgb = cameraSampleColor(x, y)
    if (rgb >= 0) {
        const r = Math.floor(rgb / 65536)
        const g = Math.floor((rgb % 65536) / 256)
        const b = Math.floor(rgb % 256)
        colorHex.set(`#${r.toString(16).padStart(2, "0")}${g.toString(16).padStart(2, "0")}${b.toString(16).padStart(2, "0")}`)
    }
})

App({
    title: "Color Picker",
    width: 400,
    height: 600,
    body: VStack(16, [
        cam,
        Text(`Color: ${colorHex.value}`),
    ]),
})
// ANCHOR_END: quick-example

// ANCHOR: camera-create
const preview = CameraView()
// ANCHOR_END: camera-create

// ANCHOR: camera-start
cameraStart(preview)
// ANCHOR_END: camera-start

// ANCHOR: camera-stop
cameraStop(preview)
// ANCHOR_END: camera-stop

// ANCHOR: camera-freeze
cameraFreeze(preview)
// ANCHOR_END: camera-freeze

// ANCHOR: camera-unfreeze
cameraUnfreeze(preview)
// ANCHOR_END: camera-unfreeze

// ANCHOR: sample-color
const rgb = cameraSampleColor(0.5, 0.5) // center of frame
// ANCHOR_END: sample-color

// ANCHOR: extract-channels
const r = Math.floor(rgb / 65536)
const g = Math.floor((rgb % 65536) / 256)
const b = Math.floor(rgb % 256)
// ANCHOR_END: extract-channels

// ANCHOR: on-tap
cameraSetOnTap(preview, (tx: number, ty: number) => {
    // tx, ty are normalized coordinates (0.0-1.0)
    const tappedRgb = cameraSampleColor(tx, ty)
    console.log(`tapped color: ${tappedRgb}`)
})
// ANCHOR_END: on-tap

// Reference each value once so the linker doesn't dead-strip the FFIs.
console.log(`extracted: r=${r} g=${g} b=${b}`)
