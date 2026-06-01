import type { JsonValue } from "./transport.js";

export interface InstanceCreateOptions {
  pluginId: string;
  classId?: string;
  sampleRate: number;
  maxBlockFrames: number;
  inputChannels: number;
  outputChannels: number;
}

export interface InstanceDescriptor {
  instanceId: number;
  streamId: number;
  pluginId: string;
  pluginPath: string;
  classId?: string;
  className?: string;
  sampleRate: number;
  maxBlockFrames: number;
  inputChannels: number;
  outputChannels: number;
  state: "allocated" | "ready" | "failed";
  workerState: "not-started" | "ready" | "failed";
  streamState: "open" | "closed";
}

export interface InstanceStatusOptions {
  instanceId: number;
}

export interface InstanceWorkerMetrics {
  ipcVersion: number;
  instances: number;
  [key: string]: JsonValue;
}

export interface InstanceStatusResult {
  instance: InstanceDescriptor;
  worker: InstanceWorkerMetrics;
}

export interface InstanceRestartOptions {
  instanceId: number;
}

export type InstanceRestartResult = InstanceStatusResult;

export interface InstanceDestroyOptions {
  instanceId: number;
}

export interface InstanceDestroyResult {
  instanceId: number;
  streamId: number;
  state: "destroyed";
}

export interface StreamLifecycleOptions {
  instanceId: number;
}

export interface InstanceApi {
  create(options: InstanceCreateOptions): Promise<InstanceDescriptor>;
  list(): Promise<InstanceDescriptor[]>;
  status(options: InstanceStatusOptions): Promise<InstanceStatusResult>;
  restart(options: InstanceRestartOptions): Promise<InstanceRestartResult>;
  destroy(options: InstanceDestroyOptions): Promise<InstanceDestroyResult>;
  openStream(options: StreamLifecycleOptions): Promise<InstanceDescriptor>;
  closeStream(options: StreamLifecycleOptions): Promise<InstanceDescriptor>;
}
