// demonstrates: async/await fetch shown in hello-world.md
// docs: docs/src/getting-started/hello-world.md
// platforms: macos, linux, windows
// run: false

async function fetchData(): Promise<string> {
    const response = await fetch("https://httpbin.org/get")
    const data = await response.json() as { origin: string }
    return data.origin
}

const ip = await fetchData()
console.log(`Your IP: ${ip}`)
