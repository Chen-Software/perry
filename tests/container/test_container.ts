import { run, create, start, stop, remove, logs, exec, pullImage, listImages, imageExists, getBackend } from 'perry/container';

async function testContainerLifecycle() {
    console.log('--- Testing Container Lifecycle ---');
    const backend = await getBackend();
    console.log(`Backend: ${backend}`);

    const image = 'alpine:latest';

    // 1. imageExists & pullImage
    if (!await imageExists(image)) {
        console.log(`Pulling ${image}...`);
        await pullImage(image);
    }
    console.log('Image is available.');

    // 2. listImages
    const images = await listImages();
    console.log(`Found ${images.length} images.`);

    // 3. create & start
    console.log('Creating container...');
    const handle = await create({
        image,
        name: 'perry-test-container',
        cmd: ['sleep', '60']
    });
    console.log(`Created: ${handle.id}`);

    console.log('Starting container...');
    await start(handle.id);

    // 4. exec
    console.log('Executing echo...');
    const result = await exec(handle.id, ['echo', 'hello-perry']);
    console.log(`Exec output: ${result.stdout.trim()}`);
    if (result.stdout.trim() !== 'hello-perry') {
        throw new Error('Exec output mismatch');
    }

    // 5. logs
    console.log('Fetching logs...');
    const containerLogs = await logs(handle.id, 10);
    console.log(`Logs received (${containerLogs.stdout.length} bytes)`);

    // 6. stop & remove
    console.log('Stopping container...');
    await stop(handle.id, 1);

    console.log('Removing container...');
    await remove(handle.id, true);

    console.log('Container lifecycle test passed!');
}

testContainerLifecycle().catch(err => {
    console.error('Test failed:', err);
    process.exit(1);
});
