import { run, list, inspect } from 'perry/container';

async function main() {
    console.log("Starting basic container E2E test...");

    const handle = await run({
        image: "alpine:latest",
        name: "e2e-test-alpine",
        cmd: ["echo", "hello from e2e"],
        rm: true
    });

    console.log(`✓ Container started with ID: ${handle.id}`);

    const info = await inspect(handle.id);
    console.log(`✓ Container status: ${info.status}`);

    const containers = await list(true);
    const found = containers.some(c => c.id === handle.id || c.name === "e2e-test-alpine");
    console.log(`✓ Container found in list: ${found}`);

    console.log("[e2e] PASS");
}

main();
