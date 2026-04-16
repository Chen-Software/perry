/**
 * perry-container-compose — Production Forgejo Stack Example
 *
 * This example demonstrates a production-ready Forgejo (self-hosted Git service)
 * deployment using Perry's container-compose API.
 *
 * Architecture:
 * - forgejo:  Main Forgejo application (codeberg.org/forgejo/forgejo)
 * - postgres: PostgreSQL database for Forgejo data
 *
 * Features:
 * - Named volumes for persistent data
 * - Custom networks for service isolation
 * - Health checks and restart policies
 * - Environment variable interpolation
 * - Proper port mapping with firewall considerations
 *
 * Run: npx tsx crates/perry-container-compose/examples/forgejo/main.ts
 */

import { composeUp, getBackend } from 'perry/container';

async function main() {
  // ──────────────────────────────────────────────────────────────
  // 1. Verify Backend Support (Required first step)
  // ──────────────────────────────────────────────────────────────

  const backend = getBackend();
  console.log(`🔧 Using container backend: ${backend}\n`);

  // ──────────────────────────────────────────────────────────────
  // 2. Define Forgejo Production Stack Configuration
  // ──────────────────────────────────────────────────────────────

  const FORGEJO_VERSION = '9.0';
  const POSTGRES_VERSION = '16-alpine';

  console.log('🚀 Deploying Forgejo stack...');

  const stack = await composeUp({
    version: '3.8',
    services: {
      postgres: {
        image: `postgres:${POSTGRES_VERSION}`,
        restart: 'always',
        environment: {
          POSTGRES_USER: '${FORGEJO_DB_USER:-forgejo}',
          POSTGRES_PASSWORD: '${FORGEJO_DB_PASSWORD:-changeme}',
          POSTGRES_DB: '${FORGEJO_DB_NAME:-forgejo}',
        },
        volumes: ['forgejo-pgdata:/var/lib/postgresql/data'],
        // Database is internal to the network, but exposed for backups
        ports: ['5432:5432'],
        networks: ['forgejo-network'],
      },
      forgejo: {
        image: `codeberg.org/forgejo/forgejo:${FORGEJO_VERSION}`,
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
          // Security and Admin
          FORGEJO__security__INSTALL_LOCK: 'true',
          FORGEJO__service__DISABLE_REGISTRATION: 'false',
          FORGEJO__service__REQUIRE_SIGNIN: 'true',
        },
        volumes: [
          'forgejo-data:/data',
          '/etc/timezone:/etc/timezone:ro',
          '/etc/localtime:/etc/localtime:ro',
        ],
        ports: [
          '3000:3000', // Web UI
          '2222:22',   // SSH
        ],
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
  // 3. Verify Stack Status
  // ──────────────────────────────────────────────────────────────

  console.log('\n🔍 Checking Forgejo stack status...\n');

  const statuses = await stack.ps();
  console.table(statuses);

  const allRunning = statuses.every((s) => s.status.toLowerCase().includes('running') || s.status.toLowerCase().includes('up'));
  if (!allRunning) {
    console.error('❌ Not all services are running!');
    console.log('Fetching logs for diagnostics...');
    const logs = await stack.logs({ service: 'forgejo', tail: 50 });
    console.log(logs.stdout);

    // Cleanup on failure
    await stack.down({ volumes: true });
    process.exit(1);
  }

  console.log('✅ Stack is up and running!');

  // ──────────────────────────────────────────────────────────────
  // 4. Health Check: Verify PostgreSQL is ready via exec
  // ──────────────────────────────────────────────────────────────

  console.log('\n🏥 Performing database health check...\n');

  try {
    const health = await stack.exec('postgres', [
      'pg_isready',
      '-U',
      '${FORGEJO_DB_USER:-forgejo}',
    ]);
    console.log('PostgreSQL Status:', health.stdout.trim());
  } catch (e) {
    console.error('❌ Database health check failed:', e);
  }

  // ──────────────────────────────────────────────────────────────
  // 5. Usage Instructions
  // ──────────────────────────────────────────────────────────────

  console.log(`
─────────────────────────────────────────────────────────────
🎉 Forgejo Stack is Ready!
─────────────────────────────────────────────────────────────

Access URLs:
  - Web UI:  http://localhost:3000
  - SSH:     ssh://localhost:2222

Environment variables used:
  FORGEJO_DB_USER=forgejo
  FORGEJO_DB_PASSWORD=changeme (change in production!)
  FORGEJO_DOMAIN=localhost

Useful stack commands:
  - View logs:    stack.logs({ service: 'forgejo' })
  - Stop stack:   stack.down()
  - Full purge:   stack.down({ volumes: true })
─────────────────────────────────────────────────────────────
`);

  // ──────────────────────────────────────────────────────────────
  // 6. Graceful Cleanup Handler
  // ──────────────────────────────────────────────────────────────

  const cleanup = async () => {
    console.log('\n🧹 Cleaning up stack...');
    // In production you might want to preserve volumes (volumes: false)
    await stack.down({ volumes: false });
    console.log('✅ Stack stopped safely');
    process.exit(0);
  };

  process.on('SIGINT', cleanup);
  process.on('SIGTERM', cleanup);

  // Keep the process alive to handle signals and keep the stack managed
  console.log('Press Ctrl+C to stop the stack.');
  await new Promise(() => {});
}

main().catch((err) => {
  console.error('Fatal error during deployment:', err);
  process.exit(1);
});
