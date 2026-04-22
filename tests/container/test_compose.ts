import { up, down, ps, logs, exec, start, stop, restart } from 'perry/compose';
import { imageExists, pullImage } from 'perry/container';

async function testComposeOrchestration() {
    console.log('--- Testing Compose Orchestration ---');

    const dbImage = 'postgres:16-alpine';
    const webImage = 'nginx:alpine';

    // 1. Pre-pull images
    for (const img of [dbImage, webImage]) {
        if (!await imageExists(img)) {
            console.log(`Pulling ${img}...`);
            await pullImage(img);
        }
    }

    // 2. composeUp
    console.log('Orchestrating stack...');
    const stack = await up({
        version: '3.8',
        services: {
            db: {
                image: dbImage,
                environment: { POSTGRES_PASSWORD: 'test' }
            },
            web: {
                image: webImage,
                dependsOn: ['db'],
                ports: ['8081:80']
            }
        }
    });
    console.log(`Stack name: ${stack.project_name}`);

    // 3. ps
    const statuses = await stack.ps();
    console.log(`Services in stack: ${statuses.length}`);
    for (const s of statuses) {
        console.log(` - ${s.name}: ${s.status}`);
    }

    // 4. exec on service
    console.log('Executing in web service...');
    const webResult = await stack.exec('web', ['nginx', '-v']);
    console.log(`Web info: ${webResult.stderr.trim()}`);

    // 5. logs from stack
    const stackLogs = await stack.logs({ tail: 5 });
    console.log('Logs collected from stack.');

    // 6. stop / start / restart
    console.log('Restarting web service...');
    await stack.restart(['web']);

    // 7. composeDown
    console.log('Tearing down stack...');
    await stack.down({ volumes: true });

    console.log('Compose orchestration test passed!');
}

testComposeOrchestration().catch(err => {
    console.error('Test failed:', err);
    process.exit(1);
});
