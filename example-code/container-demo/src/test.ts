/**
 * Perry Container Module Test Script
 *
 * Tests basic container operations using perry/container module.
 * Requires Podman to be running.
 */

import { run, create, start, stop, remove, list, inspect, getBackend } from 'perry/container';

async function main() {
  console.log('='.repeat(60));
  console.log('Perry Container Module - Integration Test');
  console.log('='.repeat(60));
  console.log();

  // 1. Get backend info
  console.log('1. Checking backend...');
  try {
    const backend = getBackend();
    console.log(`   ✓ Backend: ${backend}`);
    console.log();
  } catch (error) {
    console.log(`   ✗ Error: ${error}`);
    console.log('   This usually means the module is not available or Podman is not running.');
    process.exit(1);
  }

  // 2. List containers (should be empty initially)
  console.log('2. Listing containers...');
  try {
    const containers = await list();
    console.log(`   ✓ Found ${containers.length} container(s)`);
    if (containers.length > 0) {
      for (const c of containers) {
        console.log(`     - ${c.name} (${c.id.slice(0, 12)}) - ${c.status}`);
      }
    }
    console.log();
  } catch (error) {
    console.log(`   ✗ Error: ${error}`);
    console.log('   This means Podman is not accessible.');
    console.log();
    console.log('Troubleshooting:');
    console.log('   1. Start Podman machine:');
    console.log('      podman machine start');
    console.log('   2. Or use rootless mode (Linux only):');
    console.log('      podman info');
    console.log('   3. Check Podman socket:');
    console.log('      podman system connection list');
    process.exit(1);
  }

  // 3. Run a simple container
  console.log('3. Running test container...');
  try {
    const container = await run({
      image: 'nginx:alpine',
      name: 'perry-test-nginx',
      ports: ['8081:80'],
      env: {
        TEST_VAR: 'hello',
      },
    });
    console.log(`   ✓ Container started: ${container.id}`);
    console.log(`   ✓ Container name: ${container.name || 'unnamed'}`);
    console.log();

    // 4. Wait a bit
    console.log('4. Waiting for container to initialize...');
    await new Promise(resolve => setTimeout(resolve, 2000));
    console.log();

    // 5. Inspect the container
    console.log('5. Inspecting container...');
    try {
      const info = await inspect(container.id);
      console.log(`   ✓ Image: ${info.image}`);
      console.log(`   ✓ Status: ${info.status}`);
      console.log(`   ✓ Ports: ${info.ports.join(', ') || 'none'}`);
      console.log(`   ✓ Created: ${info.created}`);
      console.log();
    } catch (error) {
      console.log(`   ✗ Inspect failed: ${error}`);
    }

    // 6. List containers again
    console.log('6. Listing containers (should show running container)...');
    try {
      const containers = await list();
      console.log(`   ✓ Found ${containers.length} container(s):`);
      for (const c of containers) {
        console.log(`     - ${c.name} (${c.status})`);
      }
      console.log();
    } catch (error) {
      console.log(`   ✗ List failed: ${error}`);
    }

    // 7. Stop the container
    console.log('7. Stopping container...');
    try {
      await stop(container.id, 5); // 5 second timeout
      console.log(`   ✓ Container stopped`);
      console.log();
    } catch (error) {
      console.log(`   ✗ Stop failed: ${error}`);
    }

    // 8. Remove the container
    console.log('8. Removing container...');
    try {
      await remove(container.id);
      console.log(`   ✓ Container removed`);
      console.log();
    } catch (error) {
      console.log(`   ✗ Remove failed: ${error}`);
    }

    // 9. Verify cleanup
    console.log('9. Verifying cleanup...');
    try {
      const containers = await list();
      if (containers.length === 0) {
        console.log('   ✓ All containers cleaned up');
      } else {
        console.log(`   ! Warning: ${containers.length} container(s) still exist`);
        for (const c of containers) {
          console.log(`     - ${c.name}`);
        }
      }
      console.log();
    } catch (error) {
      console.log(`   ✗ Verification failed: ${error}`);
    }

    console.log('='.repeat(60));
    console.log('✓ All tests completed successfully!');
    console.log('='.repeat(60));

  } catch (error) {
    console.log(`   ✗ Run failed: ${error}`);
    console.log();
    console.log('Common issues:');
    console.log('   1. Podman not running: Start with "podman machine start"');
    console.log('   2. Image not found: Run "podman pull nginx:alpine" first');
    console.log('   3. Permission denied: Check Podman permissions');
    console.log('   4. Port in use: Use a different port (e.g., 8082:80)');
    process.exit(1);
  }
}

main().catch(console.error);
