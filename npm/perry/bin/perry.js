#!/usr/bin/env node
// Wrapper that resolves the platform-specific @perryts/perry-* binary and
// execs it with stdio inherited + argv passed through. Keeps the installed
// @perryts/perry package tiny (this script) while the native bits live in
// optional-dependency packages that npm picks by os/cpu/libc.

const { spawn } = require("child_process");

const PLATFORM_PACKAGES = {
  "darwin-arm64": "@perryts/perry-darwin-arm64",
  "darwin-x64": "@perryts/perry-darwin-x64",
  "linux-arm64": "@perryts/perry-linux-arm64",
  "linux-arm64-musl": "@perryts/perry-linux-arm64-musl",
  "linux-x64": "@perryts/perry-linux-x64",
  "linux-x64-musl": "@perryts/perry-linux-x64-musl",
  "win32-x64": "@perryts/perry-win32-x64",
};

function isMusl() {
  if (process.platform !== "linux") return false;
  try {
    const header = process.report && process.report.getReport().header;
    if (header && "glibcVersionRuntime" in header) {
      return !header.glibcVersionRuntime;
    }
  } catch (_) {}
  try {
    const release = require("fs").readFileSync("/etc/os-release", "utf8");
    return /\bID=alpine\b|\bmusl\b/i.test(release);
  } catch (_) {}
  return false;
}

function detectKey() {
  let key = `${process.platform}-${process.arch}`;
  if (process.platform === "linux" && isMusl()) key += "-musl";
  return key;
}

const key = detectKey();
const pkg = PLATFORM_PACKAGES[key];
if (!pkg) {
  console.error(
    `[perry] No prebuilt binary for ${key}.\n` +
      `Supported: ${Object.keys(PLATFORM_PACKAGES).join(", ")}\n` +
      `File an issue: https://github.com/PerryTS/perry/issues`
  );
  process.exit(1);
}

const binName = process.platform === "win32" ? "perry.exe" : "perry";
let binPath;
try {
  binPath = require.resolve(`${pkg}/bin/${binName}`);
} catch (err) {
  console.error(
    `[perry] The ${pkg} package is not installed.\n` +
      `This usually means npm skipped the optional dependency for ${key}.\n` +
      `Try: npm install --force ${pkg}\n` +
      `Or reinstall @perryts/perry with a matching npm (\u22658.12) so os/cpu/libc selectors apply.\n` +
      `Underlying error: ${err.message}`
  );
  process.exit(1);
}

const child = spawn(binPath, process.argv.slice(2), { stdio: "inherit" });

// Forward termination signals so Ctrl-C / supervisor kills propagate to perry.
for (const sig of ["SIGINT", "SIGTERM", "SIGHUP", "SIGQUIT"]) {
  process.on(sig, () => {
    try {
      child.kill(sig);
    } catch (_) {}
  });
}

child.on("close", (code, signal) => {
  if (signal) {
    // Re-raise so the parent shell sees the signal exit, not a plain 1.
    process.kill(process.pid, signal);
  } else {
    process.exit(code == null ? 0 : code);
  }
});

child.on("error", (err) => {
  console.error(`[perry] Failed to spawn ${binPath}: ${err.message}`);
  process.exit(1);
});
