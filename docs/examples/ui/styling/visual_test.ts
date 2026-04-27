// demonstrates: comprehensive visual styling + widget test (issue #185
// follow-up). One window, ~14 rows, every styling prop AND every
// commonly-used widget type. Pair with `visual_test.spec.md` for
// LLM-aided / human verification — that spec file lists every cell's
// expected color / text / layout so a tester (or model) can match
// screenshot to expectation cell-by-cell.
// docs: docs/src/ui/styling.md
// platforms: macos, linux, windows
// targets: ios-simulator, tvos-simulator, watchos-simulator, web, wasm, android

import {
    App, VStack, HStack, Text, Button, TextField, TextArea, SecureField,
    Toggle, Slider, ProgressView, ImageSymbol, Divider, Spacer,
} from "perry/ui"

// Runtime-resolved colors — exercises Phase C step 7 (js_color_parse_channel).
// Variable values prevent compile-time folding so codegen has to emit the
// runtime parseColor path.
const themeBlue = "#3B82F6"
const themePurple = "rebeccapurple"  // unsupported named — should fall through to magenta sentinel

// ── Helper: a labeled section row.
// Header text on its own line, then the body widget, vertical gap.
function labeled(title: string, body: any): any {
    return VStack(4, [
        Text(title, { color: { r: 0.4, g: 0.4, b: 0.4, a: 1 }, fontSize: 11, fontWeight: 600 }),
        body,
    ])
}

// 1. COLORS — 5 forms of color spec
const colorsRow = HStack(8, [
    Text("hex", { backgroundColor: "#FF0000", color: "white", padding: 8, borderRadius: 4 }),
    Text("named", { backgroundColor: "blue", color: "white", padding: 8, borderRadius: 4 }),
    Text("object", {
        backgroundColor: { r: 0, g: 0.6, b: 0, a: 1 },
        color: "white", padding: 8, borderRadius: 4,
    }),
    Text("alpha", { backgroundColor: "#FF000080", color: "white", padding: 8, borderRadius: 4 }),
    Text("runtime", { backgroundColor: themeBlue, color: "white", padding: 8, borderRadius: 4 }),
])

// 2. BORDERS + CORNERS — varying widths and radii
const bordersRow = HStack(8, [
    Text("1px-4r", { borderColor: "black", borderWidth: 1, borderRadius: 4, padding: 8 }),
    Text("3px-12r", { borderColor: "blue", borderWidth: 3, borderRadius: 12, padding: 8 }),
    Text("heavy", { borderColor: "red", borderWidth: 5, borderRadius: 0, padding: 8 }),
    Text("pill", { borderColor: "#888", borderWidth: 1, borderRadius: 16, padding: 8 }),
])

// 3. PADDING — uniform vs per-side
const paddingRow = HStack(8, [
    Text("p:12", { backgroundColor: "#EEE", padding: 12, borderRadius: 4 }),
    Text("p:4-24", {
        backgroundColor: "#EEE",
        padding: { top: 4, right: 24, bottom: 4, left: 24 },
        borderRadius: 4,
    }),
])

// 4. SHADOW — soft, hard, colored. Cells use a light-gray bg (not
// pure white) so the shadow is visible against the white window
// background — pure white card on white window hides the shadow
// even when it's correctly applied to the CALayer (was the actual
// bug initially observed in this row).
const shadowRow = HStack(16, [
    Text("soft", {
        backgroundColor: "#F3F4F6", color: "black", padding: 12, borderRadius: 8,
        shadow: { color: "black", blur: 12, offsetY: 4 },
    }),
    Text("hard", {
        backgroundColor: "#F3F4F6", color: "black", padding: 12, borderRadius: 8,
        shadow: { color: "black", blur: 0, offsetX: 4, offsetY: 4 },
    }),
    Text("blue", {
        backgroundColor: "#F3F4F6", color: "black", padding: 12, borderRadius: 8,
        shadow: { color: "blue", blur: 16, offsetY: 6 },
    }),
])

// 5. GRADIENT — angle variants
const gradientRow = HStack(8, [
    Text("→", {
        gradient: { angle: 90, stops: ["#3B82F6", "#8B5CF6"] },
        color: "white", padding: 12, borderRadius: 6,
    }),
    Text("↓", {
        gradient: { angle: 180, stops: ["#EF4444", "#F59E0B"] },
        color: "white", padding: 12, borderRadius: 6,
    }),
    Text("↘", {
        gradient: { angle: 135, stops: ["#10B981", "#06B6D4"] },
        color: "white", padding: 12, borderRadius: 6,
    }),
])

// 6. OPACITY — 100/75/50/25
const opacityRow = HStack(8, [
    Text("100", { backgroundColor: "#3B82F6", color: "white", padding: 8, borderRadius: 4, opacity: 1.0 }),
    Text("75", { backgroundColor: "#3B82F6", color: "white", padding: 8, borderRadius: 4, opacity: 0.75 }),
    Text("50", { backgroundColor: "#3B82F6", color: "white", padding: 8, borderRadius: 4, opacity: 0.50 }),
    Text("25", { backgroundColor: "#3B82F6", color: "white", padding: 8, borderRadius: 4, opacity: 0.25 }),
])

// 7. TYPOGRAPHY — weight, decoration, size, family
const textRow = HStack(8, [
    Text("normal", { color: "black", fontSize: 14 }),
    Text("bold", { color: "black", fontSize: 14, fontWeight: 700 }),
    Text("under", { color: "black", fontSize: 14, textDecoration: "underline" }),
    Text("strike", { color: "black", fontSize: 14, textDecoration: "strikethrough" }),
    Text("12pt", { color: "black", fontSize: 12 }),
    Text("18pt", { color: "black", fontSize: 18 }),
    Text("mono", { color: "black", fontSize: 14, fontFamily: "monospaced" }),
])

// 8. BUTTONS — variants
const buttonsRow = HStack(8, [
    Button("Default", () => {}),
    Button("Styled", () => {}, {
        backgroundColor: "#3B82F6",
        color: "white",
        borderRadius: 6,
        padding: 8,
    }),
    Button("Outlined", () => {}, {
        borderColor: "blue",
        borderWidth: 2,
        borderRadius: 6,
        padding: 8,
    }),
    Button("Disabled", () => {}, { enabled: false }),
])

// 9. INPUTS — TextField, TextArea, SecureField
const inputsRow = HStack(8, [
    TextField("Type here…", (_v: string) => {}),
    SecureField("Password", (_v: string) => {}),
])

// 10. CONTROLS — Toggle, Slider, ProgressView
const controlsRow = HStack(16, [
    Toggle("Enable", (_on: boolean) => {}),
    Slider(0, 100, (_v: number) => {}),
    ProgressView(),
])

// 11. SYMBOLS + DIVIDER
const iconsRow = HStack(16, [
    ImageSymbol("star.fill"),
    ImageSymbol("heart.fill"),
    ImageSymbol("circle.fill"),
])

// 12. STATES — visible side-by-side, hidden should NOT appear
const stateRow = HStack(8, [
    Text("visible", { backgroundColor: "#10B981", color: "white", padding: 8, borderRadius: 4 }),
    Text("hidden!", { backgroundColor: "red", color: "white", padding: 8, borderRadius: 4, hidden: true }),
    Text("opacity-0", { backgroundColor: "blue", color: "white", padding: 8, borderRadius: 4, opacity: 0 }),
])

// 13. RUNTIME-COLOR — Phase C step 7 dynamic-value path. The
// `themePurple` here is a string variable holding "rebeccapurple"
// which isn't in our named-color set, so the runtime parseColor
// returns the magenta-ish fallback. That makes the cell visually
// distinct — if it renders MAGENTA-pink the runtime path is
// working; if it renders BLUE the codegen got the variable wrong.
const runtimeRow = HStack(8, [
    Text("var-hex", { backgroundColor: themeBlue, color: "white", padding: 8, borderRadius: 4 }),
    Text("var-bad", { backgroundColor: themePurple, color: "white", padding: 8, borderRadius: 4 }),
])

App({
    title: "Perry Styling Visual Test",
    width: 880,
    height: 940,
    body: VStack(14, [
        Text("Perry Styling Visual Test", { fontSize: 20, fontWeight: 700, color: "black" }),
        Text("Each row = one prop family. See visual_test.spec.md for expected per-cell values.", {
            fontSize: 11, color: { r: 0.5, g: 0.5, b: 0.5, a: 1 },
        }),
        Divider(),
        labeled("1. Colors (hex / named / object / alpha / runtime)", colorsRow),
        labeled("2. Borders + corners (width × radius)", bordersRow),
        labeled("3. Padding (uniform vs per-side)", paddingRow),
        labeled("4. Shadow (soft / hard / colored)", shadowRow),
        labeled("5. Gradient (angle 90 / 180 / 135)", gradientRow),
        labeled("6. Opacity (100% / 75% / 50% / 25%)", opacityRow),
        labeled("7. Typography (weight / decoration / size / family)", textRow),
        labeled("8. Buttons (default / styled / outlined / disabled)", buttonsRow),
        labeled("9. Inputs (TextField, SecureField)", inputsRow),
        labeled("10. Controls (Toggle, Slider, ProgressView)", controlsRow),
        labeled("11. Image (SF Symbols)", iconsRow),
        labeled("12. States (visible / hidden / opacity 0 — last 2 should NOT appear)", stateRow),
        labeled("13. Runtime colors (var → blue) (var → magenta fallback)", runtimeRow),
    ]),
})
