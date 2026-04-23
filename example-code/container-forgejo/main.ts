/**
 * perry-container-compose — Production Forgejo Stack Example
 *
 * This example demonstrates a production-ready Forgejo (self-hosted Git service)
 * deployment using Perry's container-compose API.
 *
 * Architecture:
 * - forgejo:  Main Forgejo application
 * - postgres: PostgreSQL database for Forgejo data
 *
 * Features:
 * - Named volumes for persistent data
 * - Custom networks for service isolation
 * - Health checks and restart policies
 * - Environment variable interpolation
 */

import { composeUp, getBackend, pullImage } from 'perry/container';

// ──────────────────────────────────────────────────────────────
// Verify Backend Support
// ──────────────────────────────────────────────────────────────

const backend = getBackend();
console.log(`🔧 Using container backend: ${backend}\n`);

// ──────────────────────────────────────────────────────────────
// Forgejo Production Stack Configuration
// ──────────────────────────────────────────────────────────────

const FORGEJO_VERSION = '1.23-stable';
const postgresVersion = '16-alpine';

const forgejoImage = `codeberg.org/forgejo/forgejo:${FORGEJO_VERSION}`;
const postgresImage = `postgres:${postgresVersion}`;

// ──────────────────────────────────────────────────────────────
// Explicit Image Management (Required for Production)
// ──────────────────────────────────────────────────────────────

console.log('📥 Pulling required images...\n');

console.log(`  - ${postgresImage}...`);
await pullImage(postgresImage);

console.log(`  - ${forgejoImage}...`);
await pullImage(forgejoImage);

console.log('\n✅ All images pulled successfully.\n');

// ──────────────────────────────────────────────────────────────
// Deploy Stack
// ──────────────────────────────────────────────────────────────

// Start the stack
const stack = await composeUp({
  name: 'forgejo-prod',
  services: {
    postgres: {
      image: `postgres:${postgresVersion}`,
      restart: 'always',
      environment: {
        POSTGRES_USER: '${FORGEJO_DB_USER:-forgejo}',
        POSTGRES_PASSWORD: '${FORGEJO_DB_PASSWORD:-changeme}',
        POSTGRES_DB: '${FORGEJO_DB_NAME:-forgejo}',
      },
      volumes: [{
        type: 'volume',
        source: 'forgejo-pgdata',
        target: '/var/lib/postgresql/data'
      }],
      ports: ['5432:5432'],
      networks: ['forgejo-network'],
    },
    forgejo: {
      image: forgejoImage,
      restart: 'always',
      dependsOn: ['postgres'],
      environment: {
        // Database configuration
        FORGEJO__database__DB_TYPE: 'postgres',
        FORGEJO__database__HOST: 'postgres:5432',
        FORGEJO__database__NAME: '${FORGEJO_DB_NAME:-forgejo}',
        FORGEJO__database__USER: '${FORGEJO_DB_USER:-forgejo}',
        FORGEJO__database__PASSWD: '${FORGEJO_DB_PASSWORD:-changeme}',
        // URL configuration
        FORGEJO__server__PROTOCOL: 'http',
        FORGEJO__server__DOMAIN: 'localhost',
        FORGEJO__server__ROOT_URL: 'http://localhost:3000',
      },
      volumes: [
        { type: 'volume', source: 'forgejo-data', target: '/data' }
      ],
      ports: ['3000:3000', '2222:22'],
      networks: ['forgejo-network'],
    },
  },
  networks: {
    'forgejo-network': {
      driver: 'bridge',
    },
  },
  volumes: {
    'forgejo-pgdata': {},
    'forgejo-data': {},
  },
});

// ──────────────────────────────────────────────────────────────
// Verify Stack Status
// ──────────────────────────────────────────────────────────────

console.log('\n🔍 Checking Forgejo stack status...\n');

const statuses = await stack.ps();
console.table(statuses);

// Verify both services are running
const allRunning = statuses.every((s) => s.status.includes('Up') || s.status.includes('running'));
if (!allRunning) {
  console.error('❌ Not all services are running!');
  const logs = await stack.logs('forgejo', 50);
  console.log('Forgejo logs:\n', logs.stdout);
  await stack.down(true);
  process.exit(1);
}

console.log('✅ Stack is up and running!');

// ──────────────────────────────────────────────────────────────
// Health Check: Verify PostgreSQL is ready
// ──────────────────────────────────────────────────────────────

console.log('\n🏥 Performing health checks...\n');

const postgresHealth = await stack.exec('postgres', [
  'pg_isready',
  '-U',
  'forgejo',
  '-d',
  'forgejo',
]);

if (postgresHealth.stdout.includes('accepting connections')) {
  console.log('✅ PostgreSQL: ready');
} else {
  console.error('❌ PostgreSQL: not ready');
  console.error('stderr:', postgresHealth.stderr);
  await stack.down(true);
  process.exit(1);
}

console.log(`
─────────────────────────────────────────────────────────────
🎉 Forgejo Stack is Ready!
─────────────────────────────────────────────────────────────
Access URLs:
  - Web UI:  http://localhost:3000
  - SSH:     ssh://localhost:2222
─────────────────────────────────────────────────────────────
`);

// ──────────────────────────────────────────────────────────────
// Cleanup on SIGINT/SIGTERM
// ──────────────────────────────────────────────────────────────

const cleanup = async () => {
  console.log('\n🧹 Cleaning up stack...');
  // Use volumes: true to destroy all data, or false to preserve it
  await stack.down(true);
  console.log('✅ Cleanup complete');
  process.exit(0);
};

process.on('SIGINT', cleanup);
process.on('SIGTERM', cleanup);

console.log('\n🚀 Press Ctrl+C to stop the stack and clean up.\n');
