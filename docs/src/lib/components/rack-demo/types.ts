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

export type DemoCopy = typeof import('./copy').demoCopy.en;
