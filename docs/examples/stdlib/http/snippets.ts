// demonstrates: per-API HTTP/networking snippets shown in docs/src/stdlib/http.md
// docs: docs/src/stdlib/http.md
// platforms: macos, linux, windows
// run: false

// Each ANCHOR block below is the exact code that the http docs page renders
// inline (via {{#include ... :NAME}}). The whole file is compiled and linked
// by the doc-tests harness, so every snippet is a tested artifact — if any
// snippet drifts from the real Fastify / fetch / axios / ws API, CI fails.
//
// `run: false` because every snippet either binds a port (`server.listen`),
// hits a remote URL (axios / fetch), or opens a WebSocket — none of which
// is hermetic in CI. Compile + link is the contract here, and that catches
// the API-shape regressions we care about (e.g. the issue #125 Fastify
// async-handler regression that the sibling `fastify_json.ts` covers).

// ANCHOR: fastify-server
import fastify from "fastify"

const app = fastify()

app.get("/", async (request: any, reply: any) => {
    return { hello: "world" }
})

app.get("/users/:id", async (request: any, reply: any) => {
    const id = request.params.id
    return { id, name: "User " + id }
})

app.post("/data", async (request: any, reply: any) => {
    const body = request.body
    reply.code(201)
    return { received: body }
})

app.listen({ port: 3000 }, () => {
    console.log("Server running on port 3000")
})
// ANCHOR_END: fastify-server

// ANCHOR: fetch-api
async function fetchExamples(): Promise<void> {
    // GET request
    const response = await fetch("https://jsonplaceholder.typicode.com/posts/1")
    const data = await response.json()

    // POST request
    const result = await fetch("https://jsonplaceholder.typicode.com/posts", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ title: "hello", body: "world", userId: 1 }),
    })

    console.log(`fetch ok: ${data !== null} status=${result.status}`)
}
// ANCHOR_END: fetch-api

// ANCHOR: axios-client
import axios from "axios"

async function axiosExamples(): Promise<void> {
    const getResp = await axios.get("https://jsonplaceholder.typicode.com/users/1")
    const data = getResp.data

    const response = await axios.post("https://jsonplaceholder.typicode.com/users", {
        name: "Perry",
        email: "perry@example.com",
    })

    console.log(`axios ok: ${data !== null} status=${response.status}`)
}
// ANCHOR_END: axios-client

// ANCHOR: websocket-client
import { WebSocket } from "ws"

function wsExample(): void {
    const ws = new WebSocket("ws://localhost:8080")

    ws.on("open", () => {
        ws.send("Hello, server!")
    })

    ws.on("message", (data: any) => {
        console.log(`Received: ${data}`)
    })

    ws.on("close", () => {
        console.log("Connection closed")
    })
}
// ANCHOR_END: websocket-client

// Reference everything so unused-import elimination doesn't strip it.
const _keep = [fetchExamples, axiosExamples, wsExample]
console.log(`http-snippets: ${_keep.length}`)
