import { composeUp, getBackend, pullImage } from 'perry/container';

async function main() {
  const backend = getBackend();
  console.log(`Using container backend: ${backend}`);

  const FORGEJO_VERSION = '9.0';
  const POSTGRES_VERSION = '16-alpine';

  const forgejoImage = `codeberg.org/forgejo/forgejo:${FORGEJO_VERSION}`;
  const postgresImage = `postgres:${POSTGRES_VERSION}`;

  console.log('📥 Pulling required images...');
  await pullImage(postgresImage);
  await pullImage(forgejoImage);
  console.log('✅ Images pulled successfully');

  console.log('🚀 Deploying Forgejo stack...');

  const stack = await composeUp({
    version: '3.8',
    services: {
      postgres: {
        image: postgresImage,
        restart: 'always',
        environment: {
          POSTGRES_USER: 'forgejo',
          POSTGRES_PASSWORD: 'changeme',
          POSTGRES_DB: 'forgejo',
        },
        volumes: ['forgejo-pgdata:/var/lib/postgresql/data'],
        networks: ['forgejo-network'],
      },
      forgejo: {
        image: forgejoImage,
        restart: 'always',
        dependsOn: ['postgres'],
        environment: {
          FORGEJO__database__DB_TYPE: 'postgres',
          FORGEJO__database__HOST: 'postgres:5432',
          FORGEJO__database__NAME: 'forgejo',
          FORGEJO__database__USER: 'forgejo',
          FORGEJO__database__PASSWD: 'changeme',
          FORGEJO__server__PROTOCOL: 'http',
          FORGEJO__server__DOMAIN: 'localhost',
          FORGEJO__server__ROOT_URL: 'http://localhost:3000',
          FORGEJO__security__INSTALL_LOCK: 'true',
        },
        volumes: ['forgejo-data:/data'],
        ports: ['3000:3000'],
        networks: ['forgejo-network'],
      },
    },
    networks: {
      'forgejo-network': { driver: 'bridge' },
    },
    volumes: {
      'forgejo-pgdata': {},
      'forgejo-data': {},
    },
  });

  console.log('🔍 Checking stack status...');
  const statuses = await stack.ps();
  console.log(JSON.stringify(statuses, null, 2));

  console.log('🏥 Health check...');
  const health = await stack.exec('postgres', ['pg_isready', '-U', 'forgejo']);
  console.log('PostgreSQL:', health.stdout.trim());

  console.log('🧹 Cleaning up...');
  await stack.down({ volumes: true });
  console.log('✅ Done');
}

main().catch(console.error);
