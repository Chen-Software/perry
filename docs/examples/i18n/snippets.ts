// demonstrates: per-API i18n snippets shown in docs/src/i18n/*.md
// docs: docs/src/i18n/overview.md, interpolation.md, formatting.md
// platforms: macos, linux, windows

// Each ANCHOR block is the exact code that the i18n docs render inline (via
// {{#include ... :NAME}}). The whole file must compile *and run* cleanly
// under Perry — if any signature drifts or any wrapper falls back to its
// pre-#188 receiver-less early-out, doc-tests fails.
//
// With no perry.toml [i18n] config in scope, `t("…")` returns its key as-is
// (the i18n transform's compile-time fallback) and the format wrappers use
// the default-locale path (en) folded in by `perry_i18n_format_*_default`.

// ANCHOR: overview-imports
import { t, Currency, Percent, ShortDate, LongDate, FormatNumber, FormatTime, Raw } from "perry/i18n"
// ANCHOR_END: overview-imports

// ANCHOR: overview-ui-strings
// String literals in UI component calls are automatically localizable keys.
// (Conceptually the docs write `Button("Next")` / `Text("Hello, {name}!", ...)`
// from "perry/ui"; we exercise the runtime API that backs them — t() — here.)
const next = t("Next")                                  // Automatically localized
const hello = t("Hello, {name}!", { name: "Alice" })    // With interpolation
console.log(next, hello)
// ANCHOR_END: overview-ui-strings

// ANCHOR: interp-params
// Use {param} placeholders in your strings and pass values as a second arg.
const greeting = t("Hello, {name}!", { name: "Alice" })
const total = t("Total: {price}", { price: 23.10 })
console.log(greeting, total)
// ANCHOR_END: interp-params

// ANCHOR: interp-plural
// Reference the base key without any suffix — Perry picks the plural variant
// from the `count` parameter and the current locale's CLDR rules.
const cartCount = 3
const itemMessage = t("You have {count} items", { count: cartCount })
console.log(itemMessage)
// ANCHOR_END: interp-plural

// ANCHOR: interp-explicit-t
// For strings outside UI components (API responses, notifications, …), use t():
const message = t("Your order has been shipped.")
const welcome = t("Welcome back, {name}!", { name: "Alice" })
console.log(message, welcome)
// ANCHOR_END: interp-explicit-t

// ANCHOR: format-imports
// All format wrappers come from perry/i18n.
// (The same `Currency`, `Percent`, … you'd pass into a Text(...) param object.)
const price = Currency(23.10)
const discount = Percent(0.15)
const population = FormatNumber(1234567.89)
const due = ShortDate(Date.now())
const event = LongDate(Date.now())
const at = FormatTime(Date.now())
const code = Raw(12345)

// Compose them with t() the same way you'd compose with Text(...):
console.log(t("Total: {price}", { price }))
console.log(t("Discount: {rate}", { rate: discount }))
console.log(t("Population: {n}", { n: population }))
console.log(t("Due: {d}", { d: due }))
console.log(t("Event: {d}", { d: event }))
console.log(t("At: {t}", { t: at }))
console.log(t("Code: {amount}", { amount: code }))
// ANCHOR_END: format-imports

// ANCHOR: format-currency
// Currency: locale-appropriate symbol, separator, and placement.
//   en: "$23.10"   de: "23,10 €"   ja: "¥23.10"
const cur = Currency(23.10)
console.log(t("Total: {price}", { price: cur }))
// ANCHOR_END: format-currency

// ANCHOR: format-percent
// Percent: input is a decimal (0.15 → 15 %). en omits the space; de/fr add it.
const rate = Percent(0.15)
console.log(t("Discount: {rate}", { rate }))
// ANCHOR_END: format-percent

// ANCHOR: format-number
// FormatNumber: locale-appropriate grouping + decimal separators.
//   en: "1,234,567.89"   de: "1.234.567,89"   fr: "1 234 567,89"
const n = FormatNumber(1234567.89)
console.log(t("Population: {n}", { n }))
// ANCHOR_END: format-number

// ANCHOR: format-date
// ShortDate / LongDate take a millisecond timestamp.
const now = Date.now()
const short = ShortDate(now)   // en: "3/22/2026"   de: "22.03.2026"
const long = LongDate(now)     // en: "March 22, 2026"   de: "22. März 2026"
console.log(t("Due: {d}", { d: short }))
console.log(t("Event: {d}", { d: long }))
// ANCHOR_END: format-date

// ANCHOR: format-time
// FormatTime: 12h vs 24h based on the active locale.
const ts = Date.now()
const formatted = FormatTime(ts)
console.log(t("At: {t}", { t: formatted }))
// ANCHOR_END: format-time

// ANCHOR: format-raw
// Raw is a pass-through — prevents auto-formatting when the param name
// might otherwise trigger it.
const orderCode = Raw(12345)
console.log(t("Code: {amount}", { amount: orderCode }))
// ANCHOR_END: format-raw
