# OCI Container Management

Perry provides a unified, strongly-typed API for managing OCI containers and multi-container orchestration. This feature is backed by a platform-adaptive engine that automatically selects the best available backend (Apple Container on macOS/iOS, Podman or Docker on Linux/Windows).

## Modules

There are two primary modules for container management:

- `perry/container`: Single-container lifecycle (run, create, start, stop, remove, logs, exec, pullImage, listImages, imageExists, inspectImage).
- `perry/compose`: Multi-container orchestration (up, down, ps, logs, exec, start, stop, restart).

## Basic Usage

### Single Container

```typescript
import { run, pullImage, imageExists } from 'perry/container';

// Ensure image exists
const image = 'alpine:latest';
if (!await imageExists(image)) {
    await pullImage(image);
}

// Run an ephemeral container
const handle = await run({
    image,
    cmd: ['echo', 'Hello from Perry!'],
    rm: true
});

console.log(`Started container: ${handle.id}`);
```

### Multi-Container Orchestration

```typescript
import { composeUp } from 'perry/compose';

const stack = await composeUp({
    version: '3.8',
    services: {
        db: {
            image: 'postgres:16-alpine',
            environment: {
                POSTGRES_PASSWORD: '${DB_PASSWORD:-secret}'
            },
            volumes: ['db-data:/var/lib/postgresql/data']
        },
        web: {
            image: 'my-app:latest',
            dependsOn: ['db'],
            ports: ['3000:3000']
        }
    },
    volumes: {
        'db-data': {}
    }
});

const status = await stack.ps();
console.table(status);

// Tear down the stack
await stack.down({ volumes: true });
```

## Backend Selection

Perry automatically probes for an available container backend in the following order:

**macOS / iOS:**
1. `apple/container`
2. `orbstack`
3. `colima`
4. `rancher-desktop`
5. `podman`
6. `lima`
7. `docker`

**Linux / Windows:**
1. `podman`
2. `nerdctl`
3. `docker`

You can override the automatic detection by setting the `PERRY_CONTAINER_BACKEND` environment variable.

## Security and Verification

All image operations can be verified using Perry's built-in Sigstore/cosign integration. When running security-sensitive capabilities, Perry enforces:
- Image signature verification.
- Read-only root filesystems.
- Isolated networking (defaulting to `none` for capabilities).
- Resource limits.

## API Reference

### `perry/container`

| Function | Description |
| --- | --- |
| `run(spec: ContainerSpec)` | Pulls (if needed) and runs a container. |
| `create(spec: ContainerSpec)` | Creates a container without starting it. |
| `start(id: string)` | Starts an existing container. |
| `stop(id: string, timeout?: number)` | Stops a running container. |
| `remove(id: string, force?: boolean)` | Removes a container. |
| `logs(id: string, tail?: number)` | Retrieves container logs. |
| `exec(id: string, cmd: string[], ...)` | Executes a command in a running container. |
| `pullImage(ref: string)` | Explicitly pulls an image. |
| `imageExists(ref: string)` | Checks if an image is available locally. |
| `getBackend()` | Returns the name of the active backend. |

### `perry/compose`

| Function | Description |
| --- | --- |
| `up(spec: ComposeSpec)` | Starts a multi-container stack. |
| `down(options?: DownOptions)` | Stops and removes a stack. |
| `ps()` | Lists status of services in the stack. |
| `logs(options?: LogOptions)` | Streams or fetches logs for the stack. |
| `exec(service: string, cmd: string[])` | Runs a command in a service container. |
