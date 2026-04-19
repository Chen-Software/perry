import type { WorkerRequest, WorkerResponse } from './types';

// Worker script source — executed inside each Web Worker.
// Self-contained: no imports, no outer scope references.
const WORKER_SCRIPT = `
"use strict";
self.onmessage = function(e) {
  var msg = e.data;
  try {
    var fn = new Function("return " + msg.fn)();
    var ctx = msg.context;
    if (msg.type === "map") {
      var results = [];
      var chunk = msg.chunk;
      for (var i = 0; i < chunk.length; i++) {
        results.push(ctx !== undefined ? fn(chunk[i], ctx) : fn(chunk[i]));
      }
      self.postMessage({ type: "result", data: results });
    } else if (msg.type === "filter") {
      var filtered = [];
      var chunk = msg.chunk;
      for (var i = 0; i < chunk.length; i++) {
        if (ctx !== undefined ? fn(chunk[i], ctx) : fn(chunk[i])) {
          filtered.push(chunk[i]);
        }
      }
      self.postMessage({ type: "result", data: filtered });
    } else if (msg.type === "exec") {
      var result = ctx !== undefined ? fn(ctx) : fn();
      self.postMessage({ type: "result", data: result });
    }
  } catch (err) {
    self.postMessage({ type: "error", message: String(err), stack: err && err.stack });
  }
};
`;

// Prepended inside Node `worker_threads` workers so WORKER_SCRIPT can run
// unchanged (it expects a browser-style `self` with `onmessage`/`postMessage`).
const NODE_WORKER_PREAMBLE = `
"use strict";
var { parentPort } = require('worker_threads');
var self = { onmessage: null, postMessage: function(m) { parentPort.postMessage(m); } };
parentPort.on('message', function(data) { if (self.onmessage) self.onmessage({ data: data }); });
`;

interface WorkerLike {
  postMessage(msg: unknown): void;
  addEventListener(type: 'message' | 'error', handler: (e: any) => void): void;
  removeEventListener(type: 'message' | 'error', handler: (e: any) => void): void;
}

type WorkerFactory = () => WorkerLike;

interface PooledWorker {
  worker: WorkerLike;
  busy: boolean;
}

let pool: PooledWorker[] | null = null;
let cachedFactory: WorkerFactory | null | undefined;

function detectWorkerFactory(): WorkerFactory | null {
  // Browser / Bun: global Worker + Blob URL
  if (
    typeof Worker !== 'undefined' &&
    typeof Blob !== 'undefined' &&
    typeof URL !== 'undefined' &&
    typeof URL.createObjectURL === 'function'
  ) {
    const blob = new Blob([WORKER_SCRIPT], { type: 'text/javascript' });
    const url = URL.createObjectURL(blob);
    return () => new Worker(url) as unknown as WorkerLike;
  }

  // Node: worker_threads. Guarded try so bundling for the browser without this
  // path doesn't hard-crash at import time if `require` isn't defined.
  try {
    if (typeof require !== 'function') return null;
    const wt = require('worker_threads') as typeof import('worker_threads');
    const script = NODE_WORKER_PREAMBLE + WORKER_SCRIPT;
    return () => adaptNodeWorker(new wt.Worker(script, { eval: true }));
  } catch {
    return null;
  }
}

function adaptNodeWorker(nw: import('worker_threads').Worker): WorkerLike {
  // Map the user's handler → our wrapped Node listener so removeEventListener
  // can find and detach the right one.
  const wrappedMsg = new WeakMap<(e: any) => void, (data: unknown) => void>();
  const wrappedErr = new WeakMap<(e: any) => void, (err: Error) => void>();

  return {
    postMessage(msg) {
      nw.postMessage(msg);
    },
    addEventListener(type, handler) {
      if (type === 'message') {
        const w = (data: unknown) => handler({ data } as MessageEvent<WorkerResponse>);
        wrappedMsg.set(handler, w);
        nw.on('message', w);
      } else if (type === 'error') {
        const w = (err: Error) => handler({ message: err?.message ?? String(err) } as ErrorEvent);
        wrappedErr.set(handler, w);
        nw.on('error', w);
      }
    },
    removeEventListener(type, handler) {
      if (type === 'message') {
        const w = wrappedMsg.get(handler);
        if (w) {
          nw.off('message', w);
          wrappedMsg.delete(handler);
        }
      } else if (type === 'error') {
        const w = wrappedErr.get(handler);
        if (w) {
          nw.off('error', w);
          wrappedErr.delete(handler);
        }
      }
    },
  };
}

function getWorkerFactory(): WorkerFactory | null {
  if (cachedFactory === undefined) cachedFactory = detectWorkerFactory();
  return cachedFactory;
}

function getDefaultConcurrency(): number {
  if (typeof navigator !== 'undefined' && navigator.hardwareConcurrency) {
    return navigator.hardwareConcurrency;
  }
  try {
    if (typeof require === 'function') {
      return (require('os') as typeof import('os')).cpus().length;
    }
  } catch {
    // fall through
  }
  return 4;
}

function ensurePool(size: number): PooledWorker[] {
  const factory = getWorkerFactory();
  if (!factory) return [];
  if (pool && pool.length >= size) return pool;

  pool = pool || [];
  while (pool.length < size) {
    pool.push({ worker: factory(), busy: false });
  }
  return pool;
}

/** Returns true if Web Workers (or Node worker_threads) are available in this environment. */
export function hasWorkerSupport(): boolean {
  return getWorkerFactory() !== null;
}

export { getDefaultConcurrency };

/**
 * Send a task to a specific worker and return a promise for the result.
 */
export function dispatch(worker: WorkerLike, request: WorkerRequest): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const handler = (e: MessageEvent<WorkerResponse>) => {
      worker.removeEventListener('message', handler);
      worker.removeEventListener('error', errorHandler);
      if (e.data.type === 'error') {
        const err = new Error(e.data.message || 'Worker error');
        if (e.data.stack) (err as any).workerStack = e.data.stack;
        reject(err);
      } else {
        resolve(e.data.data);
      }
    };
    const errorHandler = (e: ErrorEvent) => {
      worker.removeEventListener('message', handler);
      worker.removeEventListener('error', errorHandler);
      reject(new Error(e.message || 'Worker error'));
    };
    worker.addEventListener('message', handler);
    worker.addEventListener('error', errorHandler);
    worker.postMessage(request);
  });
}

/**
 * Distribute chunks across the worker pool and collect results.
 */
export async function distributeChunks<T>(
  chunks: T[][],
  fn: string,
  type: 'map' | 'filter',
  context?: unknown,
): Promise<T[][]> {
  const workers = ensurePool(chunks.length);
  const promises: Promise<unknown>[] = [];

  for (let i = 0; i < chunks.length; i++) {
    const w = workers[i % workers.length];
    promises.push(dispatch(w.worker, { type, chunk: chunks[i], fn, context }));
  }

  return (await Promise.all(promises)) as T[][];
}

/**
 * Run a single function on a worker.
 */
export async function dispatchExec(fn: string, context?: unknown): Promise<unknown> {
  const workers = ensurePool(1);
  return dispatch(workers[0].worker, { type: 'exec', fn, context });
}
