// demonstrates: perry/i18n format wrappers (Currency, Percent, FormatNumber, ShortDate, LongDate, FormatTime, Raw)
// docs: docs/src/i18n/formatting.md
// platforms: macos, linux, windows

import { Currency, Percent, FormatNumber, ShortDate, LongDate, FormatTime, Raw } from "perry/i18n"

// Fixed timestamp so the output is byte-stable: 2025-03-22T00:05:00Z.
const fixed = 1742601900000

console.log(Currency(99.99))
console.log(Percent(0.42))
console.log(FormatNumber(1234567))
console.log(ShortDate(fixed))
console.log(LongDate(fixed))
console.log(FormatTime(fixed))
console.log(Raw(12345))
