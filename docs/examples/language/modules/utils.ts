// docs: docs/src/language/supported-features.md (modules)
// platforms: macos, linux, windows

// ANCHOR: exports
// Named exports
export function helper(x: number): number { return x + 1 }
export const VALUE = 42

// Default export
export default class MyClass {
    name: string
    constructor(name: string) {
        this.name = name
    }
}
// ANCHOR_END: exports
