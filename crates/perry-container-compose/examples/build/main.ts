import { up, down } from 'perry/compose';

const stack = await up({
  version: '3.8',
  services: {
    app: {
      build: {
        context: '.',
        dockerfile: 'Dockerfile',
        args: {
          BUILD_ENV: 'production',
        },
      },
      ports: ['8080:8080'],
      environment: {
        NODE_ENV: 'production',
      },
    },
  },
});

// Tear down when done
await down(stack);
