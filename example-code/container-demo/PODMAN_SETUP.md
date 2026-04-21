# Perry Container Module - Podman Setup Guide

## Problem: Podman Not Running

Your system shows:
- ✅ Podman is installed (version 5.3.2)
- ❌ Hardware virtualization not supported (No hardware virtualization)
- ❌ Podman machine cannot start

This is common on macOS, especially with Apple Silicon.

## Solutions

### Option 1: Use Colima (Recommended)

Colima provides Lima VM-based container runtime that works well on macOS and integrates with Podman:

```bash
# Install Colima
brew install colima

# Start Colima (this creates a VM and sets up Podman)
colima start

# Verify
colima status
podman info
```

Colima automatically:
- Creates a Lima VM with hardware virtualization
- Configures Podman to use the VM
- Sets up proper networking and storage

### Option 2: Use Lima VM Directly

```bash
# Install Lima
brew install lima

# Create a VM
limactl start --name=perry-dev --vm-type=vz

# Export Podman connection
eval $(limactl shell perry-dev -- sh -c 'echo "export CONTAINER_HOST=unix://$HOME/.lima/perry-dev/sock/podman.sock"')

# Test
podman run --rm nginx:alpine echo "Hello from Lima!"
```

### Option 3: Use Docker Desktop (Alternative)

If you prefer Docker Desktop, it also works as a container backend:

```bash
# Install Docker Desktop
brew install --cask docker

# Start Docker Desktop
open -a Docker

# Enable Docker socket for Podman (optional)
podman system connection add docker --default
```

### Option 4: Test on Linux (Native)

For full native performance, test on Linux:

```bash
# Using a VM (Multipass, UTM, etc.) or remote Linux server:
podman run --rm -p 8080:80 nginx:alpine
```

## Quick Start with Colima

```bash
# 1. Install and start Colima
brew install colima
colima start

# 2. Verify Podman works
podman run --rm nginx:alpine echo "Podman is working!"

# 3. Test Perry Container Module
cd example-code/container-demo
npm install
npm run build
./container-demo

# 4. Run the test
perry compile src/test.ts -o test-podman
./test-podman
```

## Verifying Podman Connection

After starting Colima (or other solution), verify:

```bash
# Check Podman info
podman info

# List containers
podman ps -a

# Run a test container
podman run --rm -p 8081:80 nginx:alpine sh -c "echo 'Container is running!' && sleep 5"

# Test Perry backend detection
podman info --format '{{.HostInfo.OperatingSystem}}'
```

You should see:
```
hostArch: arm64
os: linux
```

## Troubleshooting

### "Cannot connect to Podman"

1. **Colima not running:**
   ```bash
   colima status
   colima start
   ```

2. **Socket not found:**
   ```bash
   colima stop
   colima delete
   colima start
   ```

3. **Permission issues:**
   ```bash
   # Colima usually handles this, but check:
   colima ssh -- ls -la /var/run/podman
   ```

### "Hardware virtualization not supported"

This is a macOS limitation. Use Colima or Lima VM-based solutions.

### "Backend failed to execute"

1. **Container not found:**
   ```bash
   podman pull nginx:alpine
   ```

2. **Port already in use:**
   - Use a different port in the test script
   - Or stop the conflicting container:
     ```bash
     podman ps | grep 8081
     podman stop <container_id>
     ```

3. **Image pull failed:**
   ```bash
   podman pull nginx:alpine
   podman images
   ```

## Performance Notes

- **Colima/Lima VM**: Adds ~1-2 seconds of cold start, but good for development
- **Native Linux**: No VM overhead, best performance
- **macOS native**: Apple Container framework is planned but not yet implemented

## Testing Perry Container Module

Once Podman is working:

```bash
# Compile test
cd example-code/container-demo
perry compile src/test.ts -o test-podman

# Run test
./test-podman
```

Expected output:
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
   ✓ Ports: 0.0.0.0.8081->80/tcp
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

## Next Steps

1. Install Colima (or use Lima/Docker Desktop)
2. Start the VM: `colima start`
3. Verify Podman: `podman run --rm nginx:alpine echo "Hello!"`
4. Test Perry: `./test-podman`
5. Try the demo: `./container-demo`

## Additional Resources

- [Colima Documentation](https://github.com/abiosoft/colima)
- [Lima Documentation](https://github.com/lima-vm/lima)
- [Podman on macOS](https://docs.podman.io/en/latest/installation/macOS)
- [Perry Container Module](../../types/perry/container/index.d.ts)
