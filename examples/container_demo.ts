import { run, list, getBackend } from 'perry/container';

async function main() {
    console.log("Detected backend:", getBackend());

    try {
        console.log("Running alpine container...");
        const handle = await run({
            image: "alpine",
            cmd: ["echo", "Hello from Perry Container!"],
            rm: true
        });
        console.log("Container ID:", handle.id);

        console.log("Listing containers...");
        const containers = await list(true);
        console.log("Containers found:", containers.length);
        for (const c of containers) {
            console.log(`- ${c.name} (${c.image}): ${c.status}`);
        }

    } catch (e: any) {
        console.log("Error caught!");
        console.log("Type:", typeof e);
        console.log("Value:", e);
        if (typeof e === 'string') {
            try {
                const parsed = JSON.parse(e);
                console.log("Parsed error:", parsed);
            } catch (err) {}
        }
        console.error("Container operation failed:", e.message, "Code:", e.code);
    }
}

main();
