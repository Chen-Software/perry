/**
 * perry-container — Production Forgejo Stack Example
 *
 * This example demonstrates a production-ready Forgejo (self-hosted Git service)
 * deployment using Perry's container-compose API.
 *
 * Architecture:
 * - forgejo:  Main Forgejo application (codeberg.org/forgejo/forgejo)
 * - postgres: PostgreSQL database for Forgejo data
 *
 * Features:
 * - Explicit image pulling and verification before deployment
 * - Named volumes for persistent data
 * - Custom networks for service isolation
 * - Health checks and restart policies
 * - Environment variable interpolation
 *
 * Run: perry run example-code/forgejo-stack/main.ts
 */

import { composeUp, getBackend, pullImage, listImages, inspectImage } from 'perry/container';

// ──────────────────────────────────────────────────────────────
// Configuration Constants
// ──────────────────────────────────────────────────────────────

const FORGEJO_IMAGE = 'codeberg.org/forgejo/forgejo:1.23-stable';
const POSTGRES_IMAGE = 'postgres:16-alpine';

// ──────────────────────────────────────────────────────────────
// Verify Backend and Prepare Images
// ──────────────────────────────────────────────────────────────

const backend = getBackend();
console.log(`🔧 Using container backend: ${backend}`);

async function ensureImages() {
  console.log('\n📥 Preparing required images...');

  const imagesToEnsure = [FORGEJO_IMAGE, POSTGRES_IMAGE];

  for (const image of imagesToEnsure) {
    try {
      // Check if image exists locally
      const info = await inspectImage(image);
      console.log(`  - Image ${image} found (ID: ${info.id.substring(0, 12)})`);
    } catch (e) {
      // Image not found, pull it explicitly
      console.log(`  - Pulling ${image}...`);
      await pullImage(image);
      console.log(`  - Pulled ${image} successfully`);
    }
  }
}

await ensureImages();

// ──────────────────────────────────────────────────────────────
// Forgejo Production Stack Deployment
// ──────────────────────────────────────────────────────────────

console.log('\n🚀 Deploying Forgejo stack...');

// Define the full stack specification
const stack = await composeUp({
  version: '3.8',
  services: {
    postgres: {
      image: POSTGRES_IMAGE,
      restart: 'always',
      environment: {
        POSTGRES_USER: '${FORGEJO_DB_USER:-forgejo}',
        POSTGRES_PASSWORD: '${FORGEJO_DB_PASSWORD:-changeme}',
        POSTGRES_DB: '${FORGEJO_DB_NAME:-forgejo}',
      },
      volumes: ['forgejo-pgdata:/var/lib/postgresql/data'],
      ports: ['5432:5432'],
      networks: ['forgejo-network'],
    },
    forgejo: {
      image: FORGEJO_IMAGE,
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
        FORGEJO__server__PROTOCOL: '${FORGEJO_PROTOCOL:-http}',
        FORGEJO__server__DOMAIN: '${FORGEJO_DOMAIN:-localhost}',
        FORGEJO__server__ROOT_URL: '${FORGEJO_ROOT_URL:-http://localhost:3000}',
        // Security and registration
        FORGEJO__security__INSTALL_LOCK: 'true',
        FORGEJO__service__DISABLE_REGISTRATION: 'false',
        FORGEJO__service__REQUIRE_SIGNIN_VIEW: 'true',
      },
      volumes: [
        'forgejo-data:/data',
        'forgejo-config:/etc/gitea',
        '/etc/timezone:/etc/timezone:ro',
        '/etc/localtime:/etc/localtime:ro',
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
    'forgejo-pgdata': { driver: 'local' },
    'forgejo-data': { driver: 'local' },
    'forgejo-config': { driver: 'local' },
  },
});

// ──────────────────────────────────────────────────────────────
// Verify Stack Status
// ──────────────────────────────────────────────────────────────

console.log('\n🔍 Checking stack status...');

const statuses = await stack.ps();
console.table(statuses.map(s => ({
  Name: s.name,
  Status: s.status,
  Ports: s.ports.join(', ')
})));

// ──────────────────────────────────────────────────────────────
// Health Check: Verify PostgreSQL is ready
// ──────────────────────────────────────────────────────────────

console.log('\n🏥 Performing database health check...');

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
}

// ──────────────────────────────────────────────────────────────
// Usage Instructions
// ──────────────────────────────────────────────────────────────

console.log(`
─────────────────────────────────────────────────────────────
🎉 Forgejo Stack is Configured!

Access URLs:
  - Web UI:  http://localhost:3000
  - SSH:     ssh://localhost:2222

Initial Admin Setup:
  1. Open http://localhost:3000 in your browser.
  2. The first user to register will automatically become the admin.

Useful Commands:
  - View stack logs:   await stack.logs({ tail: 100 });
  - Stop stack:        await stack.down();
  - Destroy stack:     await stack.down({ volumes: true });
─────────────────────────────────────────────────────────────
`);

// ──────────────────────────────────────────────────────────────
// Cleanup on Process Exit
// ──────────────────────────────────────────────────────────────

const cleanup = async () => {
  console.log('\n🧹 Stopping stack...');
  try {
    await stack.down();
    console.log('✅ Stack stopped');
  } catch (e) {
    console.error('❌ Error during cleanup:', e);
  }
  process.exit(0);
};

process.on('SIGINT', cleanup);
process.on('SIGTERM', cleanup);
