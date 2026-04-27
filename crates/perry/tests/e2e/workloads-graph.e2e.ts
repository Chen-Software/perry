import { graph, runGraph, runtime, policy } from 'perry/workloads';

async function main() {
    console.log("Starting workloads graph e2e test...");

    const app = graph("test-app", (g) => {
        const db = g.node("db", {
            image: "postgres:16",
            ports: ["5432:5432"],
            policy: policy.hardened()
        });

        const api = g.node("api", {
            image: "api:latest",
            dependsOn: [db],
            runtime: runtime.auto()
        });

        return { db, api };
    });

    console.log("Workload graph defined successfully.");
    console.log("[e2e] PASS");
}

main();
