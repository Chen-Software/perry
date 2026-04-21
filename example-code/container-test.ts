import { getBackend, run, list } from 'perry/container';
import { up, ps } from 'perry/compose';

async function main() {
    console.log("🔧 Container Backend:", getBackend());

    const spec = {
        image: "alpine",
        name: "test-container",
        cmd: ["echo", "hello from perry"],
        rm: true
    };

    console.log("🚀 Running single container...");
    try {
        const handle = await run(spec);
        console.log("✅ Container started, ID:", handle.id);
    } catch (e) {
        console.log("❌ Failed to run container:", e.message);
    }

    const composeSpec = {
        name: "test-stack",
        services: {
            web: {
                image: "nginx:alpine",
                ports: ["8080:80"]
            }
        }
    };

    console.log("🚀 Starting compose stack...");
    try {
        const stack = await up(composeSpec);
        console.log("✅ Stack started, project:", stack.project_name);

        const services = await ps(stack.stack_id);
        console.log("🔍 Running services:", services.length);
        for (const s of services) {
            console.log(`  - ${s.name}: ${s.status}`);
        }
    } catch (e) {
        console.log("❌ Failed to start stack:", e.message);
    }
}

main();
