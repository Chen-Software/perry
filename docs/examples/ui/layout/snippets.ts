// demonstrates: per-API layout snippets shown in docs/src/ui/layout.md
// docs: docs/src/ui/layout.md
// platforms: macos, linux, windows

import {
    App,
    VStack, HStack, ZStack, Spacer, Divider,
    ScrollView, scrollviewSetChild,
    LazyVStack,
    NavStack, navstackPush,
    SplitView, splitViewAddChild,
    VStackWithInsets, HStackWithInsets,
    Text, Button, TextField, ImageFile, ImageSymbol,
    State,
    widgetAddChild, widgetAddChildAt, widgetClearChildren,
    widgetRemoveChild, widgetReorderChild,
    stackSetAlignment, stackSetDistribution, stackSetDetachesHidden,
    widgetMatchParentWidth, widgetMatchParentHeight, widgetSetHugging,
    widgetAddOverlay, widgetSetOverlayFrame,
    widgetSetHidden,
    setCornerRadius, widgetSetBackgroundColor,
} from "perry/ui"

const search = State("")
const noop = (): void => {}

// ANCHOR: vstack
const stack = VStack(16, [
    Text("First"),
    Text("Second"),
    Text("Third"),
])
// ANCHOR_END: vstack

// ANCHOR: hstack
const row = HStack(8, [
    Button("Cancel", noop),
    Spacer(),
    Button("OK", noop),
])
// ANCHOR_END: hstack

// ANCHOR: zstack
const layered = ZStack()
widgetAddChild(layered, ImageFile("background.png"))
widgetAddChild(layered, Text("Overlay text"))
// ANCHOR_END: zstack

// ANCHOR: scrollview
// ScrollView() takes no args; populate it with `scrollviewSetChild`.
const sv = ScrollView()
const inner = VStack(8, [Text("a"), Text("b"), Text("c")])
scrollviewSetChild(sv, inner)
// ANCHOR_END: scrollview

// ANCHOR: lazyvstack
// `render(index)` is invoked lazily — only rows in the visible rect are realized.
const lazy = LazyVStack(1000, (index: number) => Text(`Row ${index}`))
// ANCHOR_END: lazyvstack

// ANCHOR: navstack
const home = VStack(16, [
    Text("Home Screen"),
    Button("Go to Details", () => {
        navstackPush(nav, Text("Details!"), "Details")
    }),
])
const nav = NavStack()
widgetAddChild(nav, home)
// ANCHOR_END: navstack

// ANCHOR: spacer
const toolbar = HStack(8, [
    Text("Left"),
    Spacer(),
    Text("Right"),
])
// ANCHOR_END: spacer

// ANCHOR: divider
const sections = VStack(12, [
    Text("Section 1"),
    Divider(),
    Text("Section 2"),
])
// ANCHOR_END: divider

// ANCHOR: child-management
const list = VStack(16, [])
widgetAddChild(list, Text("appended"))            // append
widgetAddChildAt(list, Text("prepended"), 0)      // insert at index
widgetReorderChild(list, 1, 0)                    // move from→to
const removeMe = Text("temporary")
widgetAddChild(list, removeMe)
widgetRemoveChild(list, removeMe)                 // remove
widgetClearChildren(list)                         // remove all
// ANCHOR_END: child-management

// ANCHOR: alignment
const centered = VStack(16, [
    Text("Centered"),
    Text("Content"),
])
stackSetAlignment(centered, 9) // CenterX
// ANCHOR_END: alignment

// ANCHOR: distribution
const buttons = HStack(8, [
    Button("Cancel", noop),
    Button("OK", noop),
])
stackSetDistribution(buttons, 1) // FillEqually — both buttons get equal width
// ANCHOR_END: distribution

// ANCHOR: fill-parent
const banner = Text("Full width banner")
widgetMatchParentWidth(banner)
const banneredPage = VStack(16, [banner, Text("Normal width")])
// ANCHOR_END: fill-parent

// ANCHOR: hugging
const tight = Text("I stay small")
widgetSetHugging(tight, 750) // High priority — resist stretching

const stretchy = Text("I stretch")
widgetSetHugging(stretchy, 1) // Low priority — stretch to fill
// ANCHOR_END: hugging

// ANCHOR: overlay
// Overlay parent must be a ZStack — macOS NSView allows `addSubview` on
// any view, but GTK4 can only float children above siblings inside
// `gtk::Overlay` (which is what ZStack is backed by).
const container = ZStack()
widgetAddChild(container, VStack(16, [Text("Main content")])) // main child

const badge = Text("3")
setCornerRadius(badge, 10)
widgetSetBackgroundColor(badge, 1.0, 0.231, 0.188, 1.0) // RGBA red

widgetAddOverlay(container, badge)
widgetSetOverlayFrame(badge, 280, 10, 20, 20) // x, y, width, height
// ANCHOR_END: overlay

// ANCHOR: split-view
const split = SplitView()

const sidebar = VStack(8, [Text("Navigation"), Text("Item 1"), Text("Item 2")])
const content = VStack(16, [Text("Main Content")])

splitViewAddChild(split, sidebar)
splitViewAddChild(split, content)
// ANCHOR_END: split-view

// ANCHOR: insets-stack
// VStackWithInsets(spacing, top, left, bottom, right) — note: order is
// top/left/bottom/right (CSS-style), not top/right/bottom/left.
const card = VStackWithInsets(12, 16, 16, 16, 16)
widgetAddChild(card, Text("Padded content"))
widgetAddChild(card, Text("More content"))
// ANCHOR_END: insets-stack

// ANCHOR: detaches-hidden
const collapsible = VStack(8, [Text("Always visible"), Text("Sometimes hidden")])
stackSetDetachesHidden(collapsible, 1) // Hidden children leave no gap
// You can then toggle a child:
const sometimesHidden = Text("toggle me")
widgetSetHidden(sometimesHidden, 1) // 1 = hidden, 0 = visible
// ANCHOR_END: detaches-hidden

// ANCHOR: pattern-centered
const page = VStack(16, [Text("Title"), Text("Subtitle")])
stackSetAlignment(page, 9) // CenterX
// ANCHOR_END: pattern-centered

// ANCHOR: pattern-search-row
const searchInput = TextField("Search...", (v: string) => search.set(v))
widgetMatchParentWidth(searchInput)
const results = VStack(8, [])
const searchPage = VStack(12, [searchInput, results])
// ANCHOR_END: pattern-search-row

// ANCHOR: pattern-floating-badge
// Wrap the icon in a ZStack so the badge can float above it on every
// platform (see `// ANCHOR: overlay` for the GTK4 vs macOS rationale).
const icon = ZStack()
widgetAddChild(icon, ImageSymbol("bell"))
const dotBadge = Text("3")
widgetAddOverlay(icon, dotBadge)
widgetSetOverlayFrame(dotBadge, 20, -5, 16, 16)
// ANCHOR_END: pattern-floating-badge

// ANCHOR: pattern-toolbar
const titleBar = HStack(8, [
    Button("Back", noop),
    Spacer(),
    Text("Page Title"),
    Spacer(),
    Button("Settings", noop),
])
// ANCHOR_END: pattern-toolbar

App({
    title: "layout-snippets",
    width: 800,
    height: 600,
    body: VStack(8, [
        stack, row, layered,
        sv, lazy, nav,
        toolbar, sections, list, centered, buttons,
        banneredPage, tight, stretchy,
        container, split, card, collapsible,
        page, searchPage, icon, titleBar,
    ]),
})
