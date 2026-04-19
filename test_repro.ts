import { composeUp } from 'perry/container';
const spec: any = { services: {} };
const handle = await composeUp(spec);
await handle.down();
