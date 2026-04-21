# Perry Container Module Demo

This example demonstrates the `perry/container` module for managing OCI containers from compiled Perry applications.

## Prerequisites

### Required Backend

The `perry/container` module requires a container runtime:

**macOS / iOS:**
- Currently uses Podman (apple/container support coming soon)
- Install: `brew install podman`
- Initialize: `podman machine init && podman machine start`

**Linux:**
- Podman is the native backend
- Install: `sudo apt install podman` (Debian/Ubuntu)
  or: `sudo dnf install podman` (Fedora/RHEL)

**Windows:**
- Podman Desktop (WSL2 backend)

## Quick Start

```bash
# Install dependencies
npm install

# Compile
npm run build

# Run
./container-demo
```

## What It Does

This example demonstrates:

1. **Backend Detection**: Shows which container backend is being used
2. **Run Container**: Starts an nginx:alpine container with port mapping
3. **List Containers**: Queries and displays all running containers
4. **Inspect Container**: Retrieves detailed information about a container
5. **Stop Container**: Gracefully stops the running container
6. **Remove Container**: Removes the stopped container

## Expected Output

```
Perry Container Module Demo
=============================

Using backend: podman

Example 1: Running nginx container...
Container started: 8f2e9b3a1c2d

Example 2: Listing containers...
Found 1 container(s):
  - demo-nginx (8f2e9b3a1c2): running

Example 3: Inspecting container...
Container demo-nginx:
  Image: nginx:alpine
  Status: running
  Ports: 0.0.0.0:8080->80/tcp
  Created: 2024-04-14T12:34:56.789012345Z

Example 4: Stopping container...
Container stopped

Example 5: Removing container...
Container removed
```

## Advanced Usage

### Compose Orchestration

The `perry/container` module supports Docker Compose-like multi-container orchestration:

```typescript
import { composeUp } from 'perry/container';

const compose = await composeUp({
  version: '3.8',
  services: {
    web: {
      image: 'nginx:alpine',
      ports: ['8080:80'],
    },
    db: {
      image: 'postgres:15-alpine',
      environment: {
        POSTGRES_PASSWORD: 'example',
      },
    },
  },
});

// Get services
const services = await compose.ps();

// Stop and remove
await compose.down();
```

### Image Management

```typescript
import { pullImage, listImages, removeImage } from 'perry/container';

// Pull an image
await pullImage('alpine:latest');

// List all images
const images = await listImages();
for (const img of images) {
  console.log(`${img.repository}:${img.tag} (${img.size} bytes)`);
}

// Remove an image
await removeImage('alpine:latest');
```

### Container Logs

```typescript
import { logs } from 'perry/container';

// Get recent logs
const logs = await logs(containerId, { tail: 100 });
console.log('STDOUT:', logs.stdout);
console.log('STDERR:', logs.stderr);
```

## TypeScript Support

Full TypeScript type definitions are included:

```typescript
import type { ContainerSpec, ContainerInfo, ContainerLogs } from 'perry/container';

const spec: ContainerSpec = {
  image: 'nginx:alpine',
  name: 'my-nginx',
  ports: ['8080:80'],
  env: { ENV_VAR: 'value' },
};

const info: ContainerInfo = await inspect(spec.name);
console.log(info.status);
```

## Platform Notes

### macOS / iOS

Currently uses Podman backend. Apple Container framework support is planned.

### Linux

Native Podman backend with full feature support.

### Windows

Podman Desktop with WSL2 backend (experimental).

## Building for Different Targets

```bash
# Native binary (default)
perry compile src/main.ts -o container-demo

# macOS
perry compile src/main.ts --target macos -o container-demo-macos

# Linux
perry compile src/main.ts --target linux -o container-demo-linux

# Windows
perry compile src/main.ts --target windows -o container-demo.exe
```

## Troubleshooting

### "podman binary not found"

Install Podman:
- macOS: `brew install podman`
- Debian/Ubuntu: `sudo apt install podman`
- Fedora/RHEL: `sudo dnf install podman`

### "Backend failed to execute"

Make sure the Podman daemon is running:
```bash
# macOS
podman machine start

# Linux (user mode)
# Podman runs in rootless mode by default, no daemon needed
```

### "Permission denied"

Ensure your user is in the appropriate groups:
```bash
# Linux (if using rootless mode)
sudo usermod -aG podman $USER
```

## Further Reading

- [Perry Documentation](https://perryts.github.io/perry/)
- [Perry Container Module API](./types/perry/container/index.d.ts)
- [Podman Documentation](https://docs.podman.io/)
- [Docker Compose Reference](https://docs.docker.com/compose/)

## License

MIT
