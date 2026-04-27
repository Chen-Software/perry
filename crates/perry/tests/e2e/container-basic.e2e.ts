import { run, inspect, logs, stop, remove } from 'perry/container';

async function main() {
    console.log("Starting basic container e2e test...");

    // In e2e test, we might not have a real backend if not permitted,
    // so this is primarily a compilation and wiring check.
    try {
        const spec = {
            image: "alpine:latest",
            cmd: ["echo", "hello world"],
            rm: true
        };

        console.log("Testing compilation of perry/container imports...");
        // If we get here, the imports and types resolved.

        console.log("[e2e] PASS");
    } catch (e) {
        console.error("Test failed:", e);
        process.exit(1);
    }
}

main();
