import type {
  InstanceDescriptor,
  LoopbackMetrics,
  LoopbackSharedBuffers,
  Vst3ParameterInfo
} from '@wvst/web';

export type DemoStatus = 'idle' | 'blocked' | 'connecting' | 'connected' | 'scanning' | 'error';
export type SlotState = 'active' | 'failed' | 'removing';

export interface Prerequisites {
  sharedArrayBuffer: boolean;
  crossOriginIsolated: boolean;
  secureContext: boolean;
}

export interface PluginChoice {
  key: string;
  pluginId: string;
  classId?: string;
  label: string;
  sublabel: string;
  path: string;
  score: number;
}

export interface RackParameter {
  info: Vst3ParameterInfo;
  value: number;
  display: string;
  pending: boolean;
}

export interface RackSlot {
  id: string;
  choice: PluginChoice;
  instance: InstanceDescriptor;
  node: AudioWorkletNode;
  buffers: LoopbackSharedBuffers;
  metrics: LoopbackMetrics;
  parameters: RackParameter[];
  bypassed: boolean;
  state: SlotState;
  error?: string;
}

export interface DemoCopy {
  chooseFile: string;
  dropFile: string;
  replaceFile: string;
  noFile: string;
  connect: string;
  disconnect: string;
  scan: string;
  scanning: string;
  add: string;
  play: string;
  pause: string;
  stop: string;
  back: string;
  forward: string;
  loop: string;
  mute: string;
  unmute: string;
  endpoint: string;
  token: string;
  tokenHint: string;
  settings: string;
  prerequisites: string;
  secureContext: string;
  isolation: string;
  sharedBuffer: string;
  ready: string;
  missing: string;
  selectPlugin: string;
  emptyRack: string;
  emptyRackBody: string;
  noPlugins: string;
  connecting: string;
  connected: string;
  blocked: string;
  bridgeError: string;
  fileReady: string;
  invalidFile: string;
  mounted: string;
  removed: string;
  meters: string;
  bypass: string;
  enable: string;
  remove: string;
  up: string;
  down: string;
  status: string;
  failures: string;
  dryPath: string;
  processedPath: string;
  volume: string;
  parameters: string;
  noParameters: string;
  inputQueue: string;
  outputQueue: string;
  underflows: string;
  overflows: string;
  latency: string;
  bridgePanel: string;
  rackPanel: string;
}
