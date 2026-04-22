// demonstrates: fs roundtrip — write, stat, read, delete
// docs: docs/src/stdlib/fs.md
// platforms: macos, linux, windows

import { writeFileSync, readFileSync, existsSync, statSync, unlinkSync } from "fs"
import { tmpdir } from "os"
import { join } from "path"

// Use os.tmpdir() so the example works on Windows (no /tmp there).
const path = join(tmpdir(), "perry_fs_demo.txt")
const payload = "hello from perry\n"

writeFileSync(path, payload)

if (existsSync(path)) {
    const stat = statSync(path)
    console.log(`wrote ${stat.size} bytes`)
}

const readBack = readFileSync(path, "utf-8")
console.log(`roundtrip ok: ${readBack === payload}`)

unlinkSync(path)
console.log(`cleanup: ${!existsSync(path) ? "deleted" : "still there"}`)
