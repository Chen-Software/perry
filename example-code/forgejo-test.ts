import { up, ps, down } from 'perry/compose';

async function main() {
    console.log("--- Forgejo Production Orchestration Example ---");

    // Define a realistic multi-container spec for Forgejo with PostgreSQL
    const forgejoSpec = {
        name: "forgejo-stack",
        services: {
            db: {
                image: "postgres:16-alpine",
                environment: {
                    POSTGRES_USER: "forgejo",
                    POSTGRES_PASSWORD: "secret-password",
                    POSTGRES_DB: "forgejo"
                },
                volumes: ["forgejo-db:/var/lib/postgresql/data"]
            },
            forgejo: {
                image: "codeberg.org/forgejo/forgejo:8",
                ports: ["3000:3000", "2222:22"],
                depends_on: {
                    db: { condition: "service_started" }
                },
                environment: {
                    USER_UID: "1000",
                    USER_GID: "1000",
                    FORGEJO__database__DB_TYPE: "postgres",
                    FORGEJO__database__HOST: "db:5432",
                    FORGEJO__database__NAME: "forgejo",
                    FORGEJO__database__USER: "forgejo",
                    FORGEJO__database__PASSWD: "secret-password"
                },
                volumes: ["forgejo-data:/data"]
            }
        },
        volumes: {
            "forgejo-db": {},
            "forgejo-data": {}
        }
    };

    try {
        console.log("Orchestrating Forgejo stack...");
        const stack = await up(forgejoSpec);
        console.log(`Stack online. Project: ${stack.project_name}, ID: ${stack.stack_id}`);

        console.log("Verifying service status...");
        const services = await ps(stack.stack_id);
        for (const s of services) {
            console.log(`[${s.status.toUpperCase()}] ${s.name} (Image: ${s.image})`);
        }

        // Keep running for a moment to simulate workload
        console.log("Simulation complete. Tearing down...");
        await down(stack.stack_id, true); // true to remove volumes
        console.log("Cleanup finished.");
    } catch (e) {
        console.error("Orchestration failed:", e);
    }
}

main();
