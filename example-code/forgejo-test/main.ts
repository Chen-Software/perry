import { composeUp } from 'perry/container';

async function test() {
  console.log('Testing Forgejo orchestration...');
  const stack = await composeUp({
    name: 'forgejo',
    services: {
      db: {
        image: 'postgres:15-alpine',
        environment: {
          POSTGRES_PASSWORD: 'password',
          POSTGRES_USER: 'forgejo',
          POSTGRES_DB: 'forgejo'
        }
      },
      forgejo: {
        image: 'codeberg.org/forgejo/forgejo:1.21',
        depends_on: ['db'],
        ports: ['3000:3000'],
        environment: {
          FORGEJO__database__DB_TYPE: 'postgres',
          FORGEJO__database__HOST: 'db:5432',
          FORGEJO__database__NAME: 'forgejo',
          FORGEJO__database__USER: 'forgejo',
          FORGEJO__database__PASSWD: 'password'
        }
      }
    }
  });

  console.log('✓ Forgejo stack initialized');
  const ps = await stack.ps();
  console.log('✓ Services running: ' + ps.length);

  await stack.down();
  console.log('✓ Stack removed');
  console.log('[test] PASS');
}

test().catch(e => {
  console.error('Test failed:', e);
  process.exit(1);
});
