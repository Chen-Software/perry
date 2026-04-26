# @perryts/perry

Native TypeScript compiler. Compiles TypeScript source code directly to native executables via LLVM — no VM, no JIT warmup, no Node at runtime.

## Install

```bash
npm install -g @perryts/perry
# or one-shot
npx @perryts/perry compile hello.ts -o hello && ./hello
```

Installing picks the right prebuilt binary for your platform automatically — `@perryts/perry` declares per-platform packages as `optionalDependencies` and npm (≥8.12) selects the matching one based on `os` / `cpu` / `libc`.

## Supported platforms

| Platform | Package |
|---|---|
| macOS arm64 (Apple Silicon) | `@perryts/perry-darwin-arm64` |
| macOS x64 (Intel) | `@perryts/perry-darwin-x64` |
| Linux x64 (glibc) | `@perryts/perry-linux-x64` |
| Linux arm64 (glibc) | `@perryts/perry-linux-arm64` |
| Linux x64 (musl / Alpine) | `@perryts/perry-linux-x64-musl` |
| Linux arm64 (musl / Alpine) | `@perryts/perry-linux-arm64-musl` |
| Windows x64 | `@perryts/perry-win32-x64` |

## Host requirements

Perry produces native binaries by linking its runtime and stdlib (shipped as static archives in the platform package) into your code. That link step uses your system C toolchain, so you need:

- **macOS** — Xcode Command Line Tools (`xcode-select --install`)
- **Linux** — `gcc` or `clang` (e.g. `apt install build-essential` on Debian/Ubuntu, `apk add build-base` on Alpine)
- **Windows** — MSVC / Visual Studio Build Tools with the C++ workload

Node.js 16 or later is required for the wrapper itself.

## Usage

```bash
perry compile file.ts -o out      # compile to native binary
perry --version                   # print version
perry --help                      # full CLI reference
```

## Links

- Repository: https://github.com/PerryTS/perry
- Issues: https://github.com/PerryTS/perry/issues
- Changelog: https://github.com/PerryTS/perry/blob/main/CHANGELOG.md
