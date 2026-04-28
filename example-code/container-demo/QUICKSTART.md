# Perry Container Module - Quick Test Guide

## Status

✅ **Perry Container Module**: Successfully compiled and ready to test
✅ **Podman**: Installed (version 5.3.2)
❌ **Podman VM**: Not running (hardware virtualization not supported)

## Quick Start

### Option 1: Install Colima (Recommended)

```bash
# Install Colima
brew install colima

# Start Colima VM
colima start

# Run verification
cd example-code/container-demo
./verify-podman.sh
```

### Option 2: Use Docker Desktop (Alternative)

```bash
# Install Docker Desktop
brew install --cask docker

# Start Docker Desktop
open -a Docker

# Run verification
cd example-code/container-demo
./verify-podman.sh
```

## Run Tests

Once Podman is working:

```bash
# Navigate to demo directory
cd example-code/container-demo

# Install dependencies (if needed)
npm install

# Run verification script
./verify-podman.sh

# Run Perry container tests
npm test

# Or compile and run manually
perry compile src/test.ts -o test-podman
./test-podman

# Run the main demo
npm run build
./container-demo
```

## What the Tests Do

### verify-podman.sh

1. ✅ Checks Podman installation
2. ✅ Checks/starts Colima VM
3. ✅ Tests Podman connection
4. ✅ Pulls test image (nginx:alpine)
5. ✅ Runs quick container test
6. ✅ Cleans up

### test.ts (Perry Container Module)

1. ✅ Gets backend information
2. ✅ Lists containers
3. ✅ Runs a test container (nginx:alpine)
4. ✅ Waits for initialization
5. ✅ Inspects container details
6. ✅ Lists containers again
7. ✅ Stops the container
8. ✅ Removes the container
9. ✅ Verifies cleanup

## Expected Output

### verify-podman.sh

```
============================================================
Perry Container Module - Podman Setup & Verification
============================================================

1. Checking Podman installation...
   ✓ Podman installed: podman version 5.3.2

2. Checking Colima (recommended solution)...
   ✓ Colima installed: colima version 0.7.6

3. Checking Colima VM status...
   ✓ Colima VM is running
   ✓ Podman should be accessible

4. Testing Podman connection...
   ✓ Podman is accessible
   ✓ Host OS: linux
   ✓ Host Arch: arm64

5. Checking for test image...
   ✓ Test image exists

6. Running Podman test container...
   ✓ Test container started: 8f2e9b3a1c2d
   ✓ Container running
   Container logs:
   /docker-entrypoint.sh: /docker-entrypoint.d/10-listen-on-ipv6-by-default.sh
   /docker-entrypoint.sh: Launching /docker-entrypoint.d/20-envsubst-on-templates.sh
   /docker-entrypoint.sh: done

7. Cleaning up test container...
   ✓ Test container removed

============================================================
✓ Podman is ready for Perry Container Module!
============================================================
```

### test.ts

```
============================================================
Perry Container Module - Integration Test
============================================================

1. Checking backend...
   ✓ Backend: podman

2. Listing containers...
   ✓ Found 0 container(s)

3. Running test container...
   ✓ Container started: 8f2e9b3a1c2d
   ✓ Container name: perry-test-nginx

4. Waiting for container to initialize...

5. Inspecting container...
   ✓ Image: nginx:alpine
   ✓ Status: running
   ✓ Ports: 0.0.0.0:8081->80/tcp
   ✓ Created: 2024-04-14T12:34:56.789012345Z

6. Listing containers (should show running container)...
   ✓ Found 1 container(s):
     - perry-test-nginx (running)

7. Stopping container...
   ✓ Container stopped

8. Removing container...
   ✓ Container removed

9. Verifying cleanup...
   ✓ All containers cleaned up

============================================================
✓ All tests completed successfully!
============================================================
```

## Troubleshooting

### "Hardware virtualization not supported"

**Solution:** Use Colima or Lima VM
```bash
brew install colima
colima start
```

### "Cannot connect to Podman"

**Solution 1:** Start Colima
```bash
colima start
```

**Solution 2:** Reset Colima
```bash
colima stop
colima delete
colima start
```

**Solution 3:** Use Docker Desktop
```bash
open -a Docker
```

### "Backend failed to execute"

**Solution:** Pull the test image first
```bash
podman pull nginx:alpine
podman images
```

### "Port already in use"

**Solution:** Change port in test.ts or stop conflicting container
```bash
podman ps | grep 8081
podman stop <container_id>
```

## Advanced: Compose Orchestration Test

Once basic tests pass, try Compose:

```typescript
// Create compose-test.ts
import { composeUp } from 'perry/container';

async function main() {
  const compose = await composeUp({
    version: '3.8',
    services: {
      web: {
        image: 'nginx:alpine',
        ports: ['8080:80'],
      },
      redis: {
        image: 'redis:alpine',
        ports: ['6379:6379'],
      },
    },
  });

  console.log('Compose stack started');

  const services = await compose.ps();
  console.log('Services:', services.map(s => s.name).join(', '));

  await compose.down({ volumes: false });
  console.log('Compose stack stopped');
}

main().catch(console.error);
```

```bash
perry compose-test.ts -o compose-test
./compose-test
```

## Performance Notes

- **Colima VM**: ~1-2s cold start, good for development
- **Native Linux**: No VM overhead, best performance
- **Apple Container**: Planned for future macOS/iOS support

## Documentation

- [Podman Setup Guide](PODMAN_SETUP.md) - Detailed setup instructions
- [Full README](README.md) - Complete documentation
- [TypeScript Types](../../types/perry/container/index.d.ts) - API reference
- [Implementation Summary](../../.comate/specs/perry-container/summary.md) - Technical details

## Next Steps

1. ✅ Install and start Colima (or alternative)
2. ✅ Run `./verify-podman.sh` to verify Podman
3. ✅ Run `npm test` to test Perry Container Module
4. ✅ Try the main demo: `npm run build && ./container-demo`
5. ✅ Explore Compose orchestration
6. ✅ Read the full documentation

## Help

For issues:
1. Check [PODMAN_SETUP.md](PODMAN_SETUP.md) for detailed troubleshooting
2. Check Podman logs: `colima logs`
3. Verify Perry compilation: `cargo build --release -p perry-stdlib --features container`
4. Report bugs on GitHub

Happy containerizing! 🚀
