/**
 * Perry Workloads Module
 *
 * Orchestrate complex multi-node container graphs with level-based
 * batching and execution strategies.
 */

export interface WorkloadResources {
  cpu?: string;
  memory?: string;
}

export type RuntimeType = 'oci' | 'microvm' | 'wasm' | 'auto';

export interface RuntimeSpec {
  type: RuntimeType;
  config?: any;
  module?: string;
}

export type PolicyTier = 'default' | 'isolated' | 'hardened' | 'untrusted';

export interface PolicySpec {
  tier: PolicyTier;
  noNetwork?: boolean;
  readOnlyRoot?: boolean;
  seccomp?: boolean;
}

export type RefProjection = 'endpoint' | 'ip' | 'internal_url';

export interface WorkloadRef {
  nodeId: string;
  projection: RefProjection;
  port?: string;
}

export type WorkloadEnvValue = string | WorkloadRef;

export interface WorkloadNode {
  id: string;
  name: string;
  image?: string;
  resources?: WorkloadResources;
  ports: string[];
  env: Record<string, WorkloadEnvValue>;
  dependsOn: string[];
  runtime: RuntimeSpec;
  policy: PolicySpec;
}

export interface WorkloadEdge {
  from: string;
  to: string;
}

export interface WorkloadGraph {
  name: string;
  nodes: Record<string, WorkloadNode>;
  edges: WorkloadEdge[];
}

export type ExecutionStrategy = 'sequential' | 'max-parallel' | 'dependency-aware' | 'parallel-safe';
export type FailureStrategy = 'rollback-all' | 'partial-continue' | 'halt-graph';

export interface RunGraphOptions {
  strategy?: ExecutionStrategy;
  onFailure?: FailureStrategy;
}

export type NodeState = 'running' | 'stopped' | 'failed' | 'pending' | 'unknown';

export interface GraphStatus {
  nodes: Record<string, NodeState>;
  healthy: boolean;
  errors?: Record<string, string>;
}

export interface NodeInfo {
  nodeId: string;
  name: string;
  containerId?: string;
  state: NodeState;
  image?: string;
}

export interface ContainerLogs {
  stdout: string;
  stderr: string;
}

export interface WorkloadGraphHandle {
  down(opts?: { volumes?: boolean }): Promise<void>;
  status(): Promise<GraphStatus>;
  graph(): WorkloadGraph;
  logs(nodeId: string, opts?: { tail?: number }): Promise<ContainerLogs>;
  exec(nodeId: string, cmd: string[]): Promise<ContainerLogs>;
  ps(): Promise<NodeInfo[]>;
}

export function graph(name: string, spec: Partial<WorkloadGraph>): WorkloadGraph;
export function node(name: string, spec: Partial<WorkloadNode>): WorkloadNode;
export function runGraph(graph: WorkloadGraph, opts?: RunGraphOptions): Promise<WorkloadGraphHandle>;
export function inspectGraph(graph: WorkloadGraph): Promise<GraphStatus>;
