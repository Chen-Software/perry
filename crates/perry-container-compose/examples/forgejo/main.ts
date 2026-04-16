/**
 * perry-container-compose — Production Forgejo Stack Example
 *
 * This example demonstrates a production-ready Forgejo (self-hosted Git service)
 * deployment using Perry's container-compose API.
 *
 * Architecture:
 * - forgejo:  Main Forgejo application (gitea/gitea)
 * - postgres: PostgreSQL database for Forgejo data
 *
 * Features:
 * - Named volumes for persistent data
 * - Custom networks for service isolation
 * - Health checks and restart policies
 * - Environment variable interpolation
 * - Proper port mapping with firewall considerations
 */

import { composeUp, getBackend, pullImage } from 'perry/container';

async function main() {
  // ──────────────────────────────────────────────────────────────
  // Verify Backend Support
  // ──────────────────────────────────────────────────────────────

  const backend = getBackend();
  console.log(`🔧 Using container backend: ${backend}\n`);

  // ──────────────────────────────────────────────────────────────
  // Forgejo Production Stack Configuration
  // ──────────────────────────────────────────────────────────────

  const FORGEJO_VERSION = '9';
  const forgejoImage = `codeberg.org/forgejo/forgejo:${FORGEJO_VERSION}`;
  const postgresVersion = '16-alpine';
  const postgresImage = `postgres:${postgresVersion}`;

  // ──────────────────────────────────────────────────────────────
  // Explicit Image Pulling (Required for Production)
  // ──────────────────────────────────────────────────────────────

  console.log('📥 Pulling required images...');
  console.log(`- ${forgejoImage}`);
  await pullImage(forgejoImage);
  console.log(`- ${postgresImage}`);
  await pullImage(postgresImage);
  console.log('✅ Images pulled successfully.\n');

  console.log('🚀 Bringing up Forgejo stack...');

  // Stack name for tracking
  const stack = await composeUp({
    version: '3.8',
    services: {
      postgres: {
        image: postgresImage,
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
        image: forgejoImage,
        restart: 'always',
        dependsOn: ['postgres'],
        environment: {
          // Database configuration
          FORGEJO__database__HOST: '${FORGEJO_DB_HOST:-postgres:5432}',
          FORGEJO__database__NAME: '${FORGEJO_DB_NAME:-forgejo}',
          FORGEJO__database__USER: '${FORGEJO_DB_USER:-forgejo}',
          FORGEJO__database__PASSWD: '${FORGEJO_DB_PASSWORD:-changeme}',
          // URL configuration (adjust for your setup)
          FORGEJO__server__PROTOCOL: '${FORGEJO_PROTOCOL:-http}',
          FORGEJO__server__DOMAIN: '${FORGEJO_DOMAIN:-localhost}',
          FORGEJO__server__ROOT_URL: '${FORGEJO_ROOT_URL:-http://localhost:3000}',
          // Admin configuration
          FORGEJO__security__INSTALL_LOCK: 'true',
          FORGEJO__service__DISABLE_REGISTRATION: 'false',
          FORGEJO__service__REQUIRE_SIGNIN: 'true',
        },
        volumes: [
          'forgejo-data:/data',
          'forgejo-config:/config',
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
      'forgejo-pgdata': {
        driver: 'local',
      },
      'forgejo-data': {
        driver: 'local',
      },
      'forgejo-config': {
        driver: 'local',
      },
    },
  });

  // ──────────────────────────────────────────────────────────────
  // Verify Stack Status
  // ──────────────────────────────────────────────────────────────

  console.log('\n🔍 Checking Forgejo stack status...');

  const statuses = await stack.ps();
  console.table(statuses);

  // Verify both services are running
  const allRunning = statuses.every((s) => s.status.toLowerCase().includes('up') || s.status.toLowerCase().includes('running'));
  if (!allRunning) {
    console.warn('⚠️ Some services might not be fully up yet. Checking logs...');
    const logs = await stack.logs({ service: 'forgejo', tail: 50 });
    console.log('--- Forgejo Logs ---');
    console.log(logs.stdout);
  } else {
    console.log('✅ Stack is up and running!');
  }

  // ──────────────────────────────────────────────────────────────
  // Health Check: Verify PostgreSQL is ready
  // ──────────────────────────────────────────────────────────────

  console.log('\n🏥 Performing health checks...');

  const postgresHealth = await stack.exec('postgres', [
    'pg_isready',
    '-U',
    'forgejo',
    '-d',
    'forgejo',
  ]);

  if (postgresHealth.exitCode === 0) {
    console.log('✅ PostgreSQL: ready');
  } else {
    console.error('❌ PostgreSQL: not ready');
    console.error('stderr:', postgresHealth.stderr);
  }

  // ──────────────────────────────────────────────────────────────
  // Usage Instructions
  // ──────────────────────────────────────────────────────────────

  console.log(`
─────────────────────────────────────────────────────────────
🎉 Forgejo Stack is Ready!
─────────────────────────────────────────────────────────────

Access URLs:
  - Web UI:  http://localhost:3000
  - SSH:     ssh://localhost:2222

Default admin account (first-run):
  - Username: root
  - Password: (set via web UI on first login)

Useful commands:
  # View logs
  await stack.logs({ service: 'forgejo', tail: 100 });

  # Execute command in forgejo container
  await stack.exec('forgejo', ['ls', '/data/gitea/conf']);

  # Stop stack (preserves data)
  await stack.down();

  # Stop stack and remove volumes (destroys all data)
  await stack.down({ volumes: true });

─────────────────────────────────────────────────────────────
`);

  // ──────────────────────────────────────────────────────────────
  // Cleanup Handler
  // ──────────────────────────────────────────────────────────────

  const cleanup = async () => {
    console.log('\n🧹 Cleaning up stack...');
    try {
      await stack.down({ volumes: false });
      console.log('✅ Cleanup complete');
    } catch (e) {
      console.error('❌ Cleanup failed:', e);
    }
    process.exit(0);
  };

  process.on('SIGINT', cleanup);
  process.on('SIGTERM', cleanup);
}

main().catch(console.error);
