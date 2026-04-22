/**
 * perry/workloads — TypeScript bindings for Perry workload graphs
 */

export interface PolicySpec {
  tier: "default" | "isolated" | "hardened" | "untrusted";
  noNetwork: bool;
  readOnlyRoot: bool;
  seccomp: bool;
}

export interface RuntimeSpec {
  type: "oci" | "microVm" | "wasm" | "auto";
  config?: any;
  module?: string;
}

export interface WorkloadNode {
  id: string;
  name: string;
  image?: string;
  ports: string[];
  env: Record<string, string | any>;
  depends_on: string[];
  runtime: RuntimeSpec;
  policy: PolicySpec;
}

export interface WorkloadGraph {
  name: string;
  nodes: Record<string, WorkloadNode>;
}

export interface RunGraphOptions {
  strategy: "sequential" | "maxParallel" | "dependencyAware" | "parallelSafe";
  onFailure: "rollbackAll" | "partialContinue" | "haltGraph";
}

export function graph(name: string, spec: Record<string, WorkloadNode>): string;
export function runGraph(graphJson: string, opts: RunGraphOptions): Promise<number>;
