/**
 * perry/compose — TypeScript bindings for perry-container-compose
 *
 * Docker Compose-like experience for Apple Container, powered by Perry.
 *
 * @module perry/compose
 */

// ============ Configuration Types ============

/**
 * Build configuration for a service image.
 */
export interface Build {
  /** Build context directory (relative to compose file) */
  context?: string;
  /** Path to Dockerfile */
  dockerfile?: string;
  /** Build-time arguments */
  args?: Record<string, string>;
  /** Labels to add to the built image */
  labels?: Record<string, string>;
  /** Build target stage */
  target?: string;
  /** Network to use during build */
  network?: string;
}

/**
 * A single service definition in a Compose file.
 */
export interface Service {
  /** Container image reference */
  image?: string;
  /** Explicit container name */
  container_name?: string;
  /** Port mappings, e.g. "8080:80" */
  ports?: string[];
  /** Environment variables (map or KEY=VALUE list) */
  environment?: Record<string, string> | string[];
  /** Container labels */
  labels?: Record<string, string>;
  /** Volume mounts, e.g. "./data:/data:ro" */
  volumes?: string[];
  /** Build configuration */
  build?: Build;
  /** Service dependencies */
  depends_on?: string[] | Record<string, { condition?: string }>;
  /** Restart policy */
  restart?: "no" | "always" | "on-failure" | "unless-stopped";
  /** Override container entrypoint */
  entrypoint?: string | string[];
  /** Override container command */
  command?: string | string[];
  /** Networks this service is attached to */
  networks?: string[];
}

/**
 * Network definition in a Compose file.
 */
export interface ComposeNetwork {
  driver?: string;
  external?: boolean;
  name?: string;
}

/**
 * Volume definition in a Compose file.
 */
export interface ComposeVolume {
  driver?: string;
  external?: boolean;
  name?: string;
}

/**
 * Root Compose file structure (docker-compose.yaml / compose.yaml).
 */
export interface ComposeSpec {
  version?: string;
  services: Record<string, Service>;
  networks?: Record<string, ComposeNetwork>;
  volumes?: Record<string, ComposeVolume>;
}

// ============ Operation Result Types ============

/**
 * Status of a service container.
 */
export type ContainerStatusString = "running" | "stopped" | "not_found";

/**
 * Service status entry from the `ps` command.
 */
export interface ServiceStatus {
  /** Service name as defined in the compose file */
  service: string;
  /** Container name */
  container: string;
  /** Current container status */
  status: ContainerStatusString;
}

/**
 * Result of an exec call inside a container.
 */
export interface ExecResult {
  stdout: string;
  stderr: string;
  exitCode: number;
}

/**
 * Generic FFI result wrapper.
 */
export interface ComposeResult<T> {
  ok: boolean;
  result?: T;
  error?: string;
}

// ============ Options Types ============

export interface UpOptions {
  /** Start in detached mode (default: true) */
  detach?: boolean;
  /** Build images before starting */
  build?: boolean;
  /** Services to start (empty = all) */
  services?: string[];
  /** Remove orphaned containers */
  removeOrphans?: boolean;
}

export interface DownOptions {
  /** Remove named volumes */
  volumes?: boolean;
  /** Remove orphaned containers */
  removeOrphans?: boolean;
  /** Services to remove (empty = all) */
  services?: string[];
}

export interface LogsOptions {
  /** Follow log output */
  follow?: boolean;
  /** Number of lines to show from the end */
  tail?: number;
  /** Show timestamps */
  timestamps?: boolean;
}

export interface ExecOptions {
  /** User context */
  user?: string;
  /** Working directory */
  workdir?: string;
  /** Additional environment variables */
  env?: Record<string, string>;
}

export interface ConfigOptions {
  /** Output format: "yaml" | "json" */
  format?: "yaml" | "json";
}

// ============ API Functions ============

/**
 * Bring up services defined in a compose file.
 *
 * @param file - Path to compose file (default: "compose.yaml")
 * @param options - Up options
 *
 * @example
 * ```typescript
 * import { up } from 'perry/compose';
 * await up('compose.yaml', { detach: true });
 * ```
 */
export function up(file?: string, options?: UpOptions): Promise<void>;

/**
 * Stop and remove services.
 *
 * @param file - Path to compose file
 * @param options - Down options
 *
 * @example
 * ```typescript
 * import { down } from 'perry/compose';
 * await down('compose.yaml', { volumes: true });
 * ```
 */
export function down(file?: string, options?: DownOptions): Promise<void>;

/**
 * List service statuses.
 *
 * @param file - Path to compose file
 * @returns Array of ServiceStatus entries
 *
 * @example
 * ```typescript
 * import { ps } from 'perry/compose';
 * const statuses = await ps('compose.yaml');
 * console.table(statuses);
 * ```
 */
export function ps(file?: string): Promise<ServiceStatus[]>;

/**
 * Get logs from services.
 *
 * @param file - Path to compose file
 * @param services - Services to get logs from (empty = all)
 * @param options - Log options
 * @returns Map of service name → log output
 *
 * @example
 * ```typescript
 * import { logs } from 'perry/compose';
 * const output = await logs('compose.yaml', ['web'], { tail: 100 });
 * ```
 */
export function logs(
  file?: string,
  services?: string[],
  options?: LogsOptions
): Promise<Record<string, string>>;

/**
 * Execute a command in a running service container.
 *
 * @param file - Path to compose file
 * @param service - Service name
 * @param cmd - Command and arguments to execute
 * @param options - Exec options
 *
 * @example
 * ```typescript
 * import { exec } from 'perry/compose';
 * const result = await exec('compose.yaml', 'web', ['sh', '-c', 'ls /app']);
 * console.log(result.stdout);
 * ```
 */
export function exec(
  file: string,
  service: string,
  cmd: string[],
  options?: ExecOptions
): Promise<ExecResult>;

/**
 * Validate and display the parsed compose configuration.
 *
 * @param file - Path to compose file
 * @param options - Config options
 * @returns Validated configuration as YAML or JSON string
 *
 * @example
 * ```typescript
 * import { config } from 'perry/compose';
 * const yaml = await config('compose.yaml');
 * console.log(yaml);
 * ```
 */
export function config(file?: string, options?: ConfigOptions): Promise<string>;

/**
 * Start existing stopped services (does not create new containers).
 *
 * @param file - Path to compose file
 * @param services - Services to start (empty = all)
 */
export function start(file?: string, services?: string[]): Promise<void>;

/**
 * Stop running services (does not remove containers).
 *
 * @param file - Path to compose file
 * @param services - Services to stop (empty = all)
 */
export function stop(file?: string, services?: string[]): Promise<void>;

/**
 * Restart services.
 *
 * @param file - Path to compose file
 * @param services - Services to restart (empty = all)
 */
export function restart(file?: string, services?: string[]): Promise<void>;
