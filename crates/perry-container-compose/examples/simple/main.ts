import { up, down, ps } from 'perry/compose';

const stack = await up({
  version: '3.8',
  services: {
    web: {
      image: 'nginx:alpine',
      containerName: 'simple-nginx',
      ports: ['8080:80'],
      labels: {
        app: 'simple-nginx',
      },
    },
  },
});

const statuses = await ps(stack);
console.table(statuses);

// Tear down when done
await down(stack);
