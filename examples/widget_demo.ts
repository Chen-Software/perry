// Perry Widget Extension Demo
//
// This example shows how to create widgets using TypeScript that compile to
// native WidgetKit (iOS/watchOS), Glance (Android), and Tiles (Wear OS).
// Perry compiles the render tree to platform-specific source code at compile time —
// no runtime, no bridge, no JS engine in the widget extension.
//
// Compile for each platform:
//   perry examples/widget_demo.ts --target ios-widget --app-bundle-id com.example.myapp -o widget_out
//   perry examples/widget_demo.ts --target android-widget --app-bundle-id com.example.myapp -o widget_out
//   perry examples/widget_demo.ts --target watchos-widget --app-bundle-id com.example.myapp -o widget_out
//   perry examples/widget_demo.ts --target wearos-tile --app-bundle-id com.example.myapp -o widget_out
//
// Supported widgets:
//   Text(content, { font, fontWeight, color, padding, ... })
//   VStack/HStack/ZStack({ spacing }, [children], { padding, background, ... })
//   Image({ systemName: "sf.symbol.name" })
//   Spacer(), Divider()
//   ForEach(entry.items, (item) => ...)
//   Label("text", { systemImage: "star.fill" })
//   Gauge(value, { label, style: "circular" | "linear" })
//   Conditional: condition ? WidgetA : WidgetB

import { Widget, Text, VStack, HStack, Image, Spacer, ForEach, Divider, Label } from "perry/widget";

// --- Example 1: Static widget (no data fetching) ---

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

// --- Example 2: Widget with config + provider + ForEach ---

export const TopSitesWidget = Widget({
  kind: "TopSitesWidget",
  displayName: "Top Sites",
  description: "Your top performing sites",
  supportedFamilies: ["systemSmall", "systemMedium", "systemLarge"],
  appGroup: "group.io.searchbird.shared",

  config: {
    sortBy: { type: "enum", values: ["clicks", "impressions", "ctr", "position"], default: "clicks", title: "Sort By" },
    dateRange: { type: "enum", values: ["7d", "28d", "90d"], default: "7d", title: "Date Range" },
  },

  provider: async (config: { sortBy: string; dateRange: string }) => {
    const res = await fetch(`https://app.searchbird.io/api/sites?days=${config.dateRange}`)
    const data = await res.json()
    return {
      entries: [{ sites: data.sites.slice(0, 5), totalClicks: data.totalClicks }],
      reloadPolicy: { after: { minutes: 30 } }
    }
  },

  placeholder: { sites: [], totalClicks: 0 },

  render(entry: { sites: { url: string; clicks: number }[]; totalClicks: number }, family: string) {
    if (family === "systemSmall") {
      return VStack([
        Text(`${entry.totalClicks}`, { font: "title", fontWeight: "bold" }),
        Text("Total Clicks", { font: "caption", color: "secondary" }),
      ], { url: "searchbird://sites" })
    }
    return VStack([
      HStack([Text("Top Sites", { font: "headline" }), Spacer()]),
      Divider(),
      ForEach(entry.sites, (site: { url: string; clicks: number }) =>
        HStack([
          Text(site.url, { font: "body", lineLimit: 1 }),
          Spacer(),
          Text(`${site.clicks}`, { font: "caption", color: "secondary" }),
        ], { url: `searchbird://site/${site.url}` })
      ),
    ], { padding: 16 })
  },
});
