/**
 * Perry Container Module Example
 *
 * Demonstrates basic container operations using perry/container module.
 *
 * Compile: perry compile src/main.ts -o container-demo
 * Run: ./container-demo
 */

import { run, create, start, stop, remove, list, inspect, getBackend } from 'perry/container';

async function main() {
  console.log('Perry Container Module Demo');
  console.log('=============================\n');

  // Get current backend
  const backend = getBackend();
  console.log(`Using backend: ${backend}\n`);

  // Example 1: Run a simple container
  console.log('Example 1: Running nginx container...');
  try {
    const nginx = await run({
      image: 'nginx:alpine',
      name: 'demo-nginx',
      ports: ['8080:80'],
      rm: true,
    });
    console.log(`Container started: ${nginx.id}\n`);

    // Wait a bit
    await new Promise(resolve => setTimeout(resolve, 2000));

    // List containers
    console.log('Example 2: Listing containers...');
    const containers = await list();
    console.log(`Found ${containers.length} container(s):`);
    for (const c of containers) {
      console.log(`  - ${c.name} (${c.id.slice(0, 12)}): ${c.status}`);
    }
    console.log('');

    // Inspect our container
    console.log('Example 3: Inspecting container...');
    const info = await inspect(nginx.id);
    console.log(`Container ${info.name}:`);
    console.log(`  Image: ${info.image}`);
    console.log(`  Status: ${info.status}`);
    console.log(`  Ports: ${info.ports.join(', ')}`);
    console.log(`  Created: ${info.created}`);
    console.log('');

    // Stop and remove the container
    console.log('Example 4: Stopping container...');
    await stop(nginx.id);
    console.log('Container stopped\n');

    console.log('Example 5: Removing container...');
    await remove(nginx.id);
    console.log('Container removed\n');

  } catch (error) {
    console.error('Error:', error);
    console.log('\nNote: Make sure Podman is installed and running on your system.');
    console.log('On macOS: brew install podman && podman machine init && podman machine start');
    console.log('On Linux: sudo apt install podman');
  }

  // Example 6: Compose orchestration (requires more complete implementation)
  /*
  console.log('Example 6: Compose orchestration...');
  try {
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

    console.log('Compose stack started');
    const services = await compose.ps();
    console.log(`Services: ${services.length}`);

    await compose.down();
    console.log('Compose stack stopped');
  } catch (error) {
    console.error('Compose error:', error);
  }
  */
}

main().catch(console.error);
