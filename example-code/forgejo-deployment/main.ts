/**
 * perry/container — Production Forgejo Stack Example
 *
 * Demonstrates a production-ready Forgejo (self-hosted Git service)
 * deployment using Perry's perry/compose orchestration API.
 *
 * Features:
 * - Named volumes for persistent data
 * - Custom networks for service isolation
 * - Restart policies + dependency ordering (forgejo waits on postgres)
 * - Environment-variable interpolation in the YAML literal
 * - Pre-flight backend detection (apple/container, podman, docker, …)
 *
 * The compose stack is started via `up()` (returns an opaque
 * `ComposeHandle`), inspected via `exec()` for a postgres readiness probe,
 * and torn down via `down()` on SIGINT / SIGTERM. The handle-as-first-arg
 * pattern (rather than method-chaining `stack.exec(...)`) is the canonical
 * TS surface — see SPEC §C4: method-chain sugar is reserved for a future
 * TS-library wrapper layer.
 */

import { up, down, exec } from 'perry/compose';
import { getBackend } from 'perry/container';

async function main() {
  console.log(`🔧 Using container backend: ${getBackend()}\n`);

  // codeberg.org's Forgejo registry intermittently returns "unauthorized:
  // reqPackageAccess" for public image pulls; using the Gitea upstream
  // (which Forgejo forked from and stays config-compatible with) keeps the
  // example reproducible regardless of registry-side auth flakiness. The
  // env-var names below use the `GITEA_*` prefix that both projects honor.
  const GITEA_VERSION = '1.23';
  const POSTGRES_VERSION = '16-alpine';

  console.log('🚀 Deploying Gitea (Forgejo-compatible) stack...');

  const stack = await up({
    version: '3.8',
    services: {
      postgres: {
        image: `postgres:${POSTGRES_VERSION}`,
        restart: 'always',
        environment: {
          POSTGRES_USER:     '${FORGEJO_DB_USER:-forgejo}',
          POSTGRES_PASSWORD: '${FORGEJO_DB_PASSWORD:-changeme}',
          POSTGRES_DB:       '${FORGEJO_DB_NAME:-forgejo}',
        },
        volumes: ['forgejo-pgdata:/var/lib/postgresql/data'],
        ports: ['5432:5432'],
        networks: ['forgejo-network'],
      },
      forgejo: {
        image: `gitea/gitea:${GITEA_VERSION}`,
        restart: 'always',
        depends_on: ['postgres'],
        environment: {
          GITEA__database__DB_TYPE: 'postgres',
          GITEA__database__HOST:    '${FORGEJO_DB_HOST:-postgres:5432}',
          GITEA__database__NAME:    '${FORGEJO_DB_NAME:-forgejo}',
          GITEA__database__USER:    '${FORGEJO_DB_USER:-forgejo}',
          GITEA__database__PASSWD:  '${FORGEJO_DB_PASSWORD:-changeme}',
          GITEA__server__PROTOCOL:  '${FORGEJO_PROTOCOL:-http}',
          GITEA__server__DOMAIN:    '${FORGEJO_DOMAIN:-localhost}',
          GITEA__server__ROOT_URL:  '${FORGEJO_ROOT_URL:-http://localhost:3000}',
          GITEA__security__INSTALL_LOCK:           'true',
          GITEA__service__DISABLE_REGISTRATION:    'false',
          GITEA__service__REQUIRE_SIGNIN_VIEW:     'true',
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
      'forgejo-network': { driver: 'bridge' },
    },
    volumes: {
      'forgejo-pgdata': { driver: 'local' },
      'forgejo-data':   { driver: 'local' },
      'forgejo-config': { driver: 'local' },
    },
  });

  console.log('✅ Stack is up.');

  // ──────────────────────────────────────────────────────────────
  // Health Check: Verify PostgreSQL is ready
  // ──────────────────────────────────────────────────────────────
  // postgres needs ~5–10s to initialise on first run (initdb +
  // listener bind). `up()` returns as soon as the container is
  // started, not when the service inside is ready, so we poll
  // `pg_isready` until it returns 0 or we hit the timeout.
  console.log('\n🏥 Waiting for PostgreSQL to accept connections...');

  const deadline = Date.now() + 30_000;
  let pgReady = false;
  while (Date.now() < deadline) {
    try {
      await exec(stack, 'postgres', [
        'pg_isready', '-U', 'forgejo', '-d', 'forgejo',
      ]);
      pgReady = true;
      break;
    } catch (_e) {
      // pg_isready exits non-zero while the server is still booting;
      // sleep briefly and retry.
      await new Promise((r) => setTimeout(r, 1000));
    }
  }

  if (!pgReady) {
    console.error('❌ PostgreSQL did not become ready within 30s — tearing down.');
    await down(stack, { volumes: true });
    process.exit(1);
  }
  console.log('✅ PostgreSQL ready.');

  console.log(`
─────────────────────────────────────────────────────────────
🎉 Forgejo Stack is Ready!
─────────────────────────────────────────────────────────────

Access URLs:
  - Web UI:  http://localhost:3000
  - SSH:     ssh://localhost:2222

Stop with Ctrl-C (will run \`down(stack, { volumes: true })\`).
─────────────────────────────────────────────────────────────
`);

  // ──────────────────────────────────────────────────────────────
  // Cleanup on SIGINT/SIGTERM
  // ──────────────────────────────────────────────────────────────
  const cleanup = async () => {
    console.log('\n🧹 Cleaning up stack...');
    await down(stack, { volumes: true });
    console.log('✅ Cleanup complete');
    process.exit(0);
  };

  process.on('SIGINT', cleanup);
  process.on('SIGTERM', cleanup);
}

main().catch((err) => {
  console.error('💥 Fatal error:', err);
  process.exit(1);
});
