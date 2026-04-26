// demonstrates: ES module imports / exports / re-exports (docs/src/language/supported-features.md)
// docs: docs/src/language/supported-features.md
// platforms: macos, linux, windows

// ANCHOR: imports
// Default + named imports from a sibling module
import MyClass, { helper, VALUE } from "./utils"

// Re-export
export { helper } from "./utils"
// ANCHOR_END: imports

const obj = new MyClass("perry")
console.log(`name=${obj.name} val=${VALUE} helped=${helper(1)}`)
