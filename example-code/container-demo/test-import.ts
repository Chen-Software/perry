/**
 * Quick test to verify perry/container module can be imported
 */

import { run, create, start, stop, remove, list, inspect, getBackend } from 'perry/container';

console.log('Successfully imported perry/container module');
console.log('Available functions:', {
  run: typeof run,
  create: typeof create,
  start: typeof start,
  stop: typeof stop,
  remove: typeof remove,
  list: typeof list,
  inspect: typeof inspect,
  getBackend: typeof getBackend,
});

console.log('Backend:', getBackend());
