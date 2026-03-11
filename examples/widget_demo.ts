// Perry Widget Extension Demo
//
// This example shows how to create an iOS WidgetKit widget using TypeScript.
// Perry compiles the render tree to native SwiftUI source code at compile time —
// no runtime, no bridge, no JS engine in the widget extension.
//
// Compile:
//   perry examples/widget_demo.ts --target ios-widget --app-bundle-id com.example.myapp -o widget_out
//
// The output directory will contain:
//   - StockPrice.swift  (generated SwiftUI: Entry, View, Provider, WidgetBundle)
//   - Info.plist        (WidgetKit extension manifest)
//
// Build the widget extension with Xcode or swiftc:
//   xcrun --sdk iphoneos swiftc -target arm64-apple-ios17.0 \
//     widget_out/StockPrice.swift \
//     -framework WidgetKit -framework SwiftUI \
//     -o widget_out/WidgetExtension
//
// Supported widgets:
//   Text(content, { font, fontWeight, color, padding, ... })
//   VStack/HStack/ZStack({ spacing }, [children], { padding, background, ... })
//   Image({ systemName: "sf.symbol.name" })
//   Spacer()
//   Ternary conditionals: condition ? WidgetA : WidgetB

import { Widget, Text, VStack, HStack, Image, Spacer } from "perry/widget";

export const StockWidget = Widget({
  kind: "com.perry.StockPrice",
  displayName: "Stock Price",
  description: "Shows the latest stock price",
  supportedFamilies: ["systemSmall", "systemMedium"],

  entryFields: {
    symbol: "string",
    price: "number",
    change: "number",
    isPositive: "boolean",
  },

  render: (entry: { symbol: string; price: number; change: number; isPositive: boolean }) =>
    VStack({ spacing: 8 }, [
      HStack([
        Text(entry.symbol, { font: "headline", fontWeight: "bold" }),
        Spacer(),
        Image({ systemName: "chart.line.uptrend.xyaxis" }),
      ]),
      Text(`$${entry.price}`, { font: "title", fontWeight: "semibold" }),
      entry.isPositive
        ? Text(`+${entry.change}`, { color: "green", font: "caption" })
        : Text(`${entry.change}`, { color: "red", font: "caption" }),
    ], { padding: 16 }),
});
