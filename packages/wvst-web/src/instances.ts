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
  state: "allocated" | "ready";
  workerState: "not-started" | "ready" | "failed";
}

export interface InstanceDestroyOptions {
  instanceId: number;
}

export interface InstanceDestroyResult {
  instanceId: number;
  streamId: number;
  state: "destroyed";
}

export interface InstanceApi {
  create(options: InstanceCreateOptions): Promise<InstanceDescriptor>;
  list(): Promise<InstanceDescriptor[]>;
  destroy(options: InstanceDestroyOptions): Promise<InstanceDestroyResult>;
}
