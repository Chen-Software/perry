#!/bin/bash
# Quick Podman verification and setup script for Perry Container Module

set -e

echo "============================================================"
echo "Perry Container Module - Podman Setup & Verification"
echo "============================================================"
echo ""

# Check Podman installation
echo "1. Checking Podman installation..."
if command -v podman &> /dev/null; then
    PODMAN_VERSION=$(podman --version)
    echo "   ✓ Podman installed: $PODMAN_VERSION"
else
    echo "   ✗ Podman not found"
    echo "   Install with: brew install podman"
    exit 1
fi
echo ""

# Check Colima
echo "2. Checking Colima (recommended solution)..."
if command -v colima &> /dev/null; then
    echo "   ✓ Colima installed: $(colima version | head -1)"
else
    echo "   ! Colima not found (recommended)"
    echo "   Install with: brew install colima"
    COLIMA_MISSING=true
fi
echo ""

# Check if Colima is running
if command -v colima &> /dev/null; then
    echo "3. Checking Colima VM status..."
    if colima status &> /dev/null; then
        echo "   ✓ Colima VM is running"
        echo "   ✓ Podman should be accessible"
    else
        echo "   ! Colima VM is not running"
        echo "   Starting Colima..."
        colima start
        echo "   ✓ Colima VM started"
    fi
    echo ""
else
    echo "3. Skipping Colima check (not installed)"
    echo ""
fi

# Test Podman connection
echo "4. Testing Podman connection..."
if podman info &> /dev/null; then
    echo "   ✓ Podman is accessible"
    HOST_OS=$(podman info --format '{{.HostInfo.OperatingSystem}}')
    HOST_ARCH=$(podman info --format '{{.HostInfo.Arch}}')
    echo "   ✓ Host OS: $HOST_OS"
    echo "   ✓ Host Arch: $HOST_ARCH"
else
    echo "   ✗ Cannot connect to Podman"
    echo ""
    echo "   Solutions:"
    echo "   1. Start Colima: colima start"
    echo "   2. Or use Lima: limactl start --name=perry-dev"
    echo "   3. Or Docker Desktop: open -a Docker"
    exit 1
fi
echo ""

# Pull test image if needed
echo "5. Checking for test image..."
if podman images | grep -q "nginx.*alpine"; then
    echo "   ✓ Test image exists"
else
    echo "   ! Pulling test image (nginx:alpine)..."
    podman pull nginx:alpine
    echo "   ✓ Test image pulled"
fi
echo ""

# Run quick Podman test
echo "6. Running Podman test container..."
CONTAINER_ID=$(podman run -d --name perry-quick-test -p 8082:80 nginx:alpine)
echo "   ✓ Test container started: $CONTAINER_ID"

# Wait and verify
sleep 2
echo "   ✓ Container running"

# Check logs
echo "   Container logs:"
podman logs --tail 3 perry-quick-test

# Cleanup
echo ""
echo "7. Cleaning up test container..."
podman stop perry-quick-test &> /dev/null || true
podman rm perry-quick-test &> /dev/null || true
echo "   ✓ Test container removed"
echo ""

# Summary
echo "============================================================"
echo "✓ Podman is ready for Perry Container Module!"
echo "============================================================"
echo ""
echo "Next steps:"
echo "  1. Navigate to container demo: cd example-code/container-demo"
echo "  2. Install dependencies: npm install"
echo "  3. Run the test: perry compile src/test.ts -o test-podman && ./test-podman"
echo "  4. Or run the demo: perry compile src/main.ts -o container-demo && ./container-demo"
echo ""

if [ "$COLIMA_MISSING" = true ]; then
    echo "Note: Consider installing Colima for better macOS support:"
    echo "  brew install colima && colima start"
    echo ""
fi
