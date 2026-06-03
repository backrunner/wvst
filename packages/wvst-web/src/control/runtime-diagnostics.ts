import type {
  JsonValue,
  Vst3ComponentHandlerEventKind,
  Vst3RestartFlags,
} from "../client/transport.js";

export interface RuntimeTailInfo {
  samples: number;
  kind: "none" | "finite" | "infinite";
  finiteSamples?: number;
}

export interface RuntimeCapabilities {
  schemaVersion: number;
  binaryAudioProcess: boolean;
  componentState: boolean;
  controller: boolean;
  controllerState: boolean;
  parameters: boolean;
  parameterAutomation: boolean;
  units: boolean;
  unitProgramData: boolean;
  programListData: boolean;
  unitData: boolean;
  midiMapping: boolean;
  outputEvents: boolean;
  outputParameterChanges: boolean;
  componentHandlerEvents: boolean;
  connectionPoints: boolean;
  processContext: boolean;
  unavailable?: RuntimeCapabilityDiagnostic[];
}

export interface RuntimeCapabilityDiagnostic {
  capability:
    | "component-state"
    | "controller"
    | "controller-state"
    | "parameters"
    | "units"
    | "unit-program-data"
    | "program-list-data"
    | "unit-data"
    | "midi-mapping"
    | "component-handler-events"
    | "connection-points"
    | "process-context"
    | "output-events"
    | "output-parameter-changes";
  reason:
    | "controller-unavailable"
    | "interface-unavailable"
    | "feature-unavailable"
    | "probe-failed";
  message?: string;
  hint: string;
}

export interface InstanceWorkerMetrics {
  ipcVersion: number;
  instances: number;
  processingInstances: number;
  runtime?: InstanceWorkerRuntimeMetrics[];
  [key: string]: JsonValue | InstanceWorkerRuntimeMetrics[] | undefined;
}

export interface InstanceWorkerRuntimeMetrics {
  streamId: number;
  backend: string;
  runtimeCapabilities: RuntimeCapabilities;
  latencySamples: number;
  tailSamples: number;
  tailInfo: RuntimeTailInfo;
  diagnostics?: InstanceWorkerRuntimeDiagnostics;
}

export interface InstanceWorkerRuntimeDiagnostics {
  componentHandler?: Vst3ComponentHandlerSnapshot | null;
  connectionPoints?: Vst3ConnectionPointDiagnostics | null;
  processContextRequirements?: number;
  audioBuses?: Vst3WorkerAudioBusDiagnostics | null;
}

export interface Vst3ConnectionPointDiagnostics {
  connected: boolean;
}

export interface Vst3WorkerAudioBusDiagnostics {
  input?: Vst3WorkerSelectedAudioBus | null;
  output: Vst3WorkerSelectedAudioBus;
}

export interface Vst3WorkerSelectedAudioBus {
  direction: "input" | "output";
  requestedChannels: number;
  requestedIndex?: number;
  selectedIndex: number;
  selected?: Vst3WorkerAudioBusInfo | null;
  available: Vst3WorkerAudioBusInfo[];
}

export interface Vst3WorkerAudioBusInfo {
  index: number;
  direction: "input" | "output";
  channelCount: number;
  busType: Vst3WorkerAudioBusType;
  defaultActive: boolean;
  controlVoltage: boolean;
  name?: string;
}

export type Vst3WorkerAudioBusType =
  | { kind: "main" }
  | { kind: "aux" }
  | { kind: "unknown"; raw: number };

export interface Vst3ComponentHandlerSnapshot {
  totalEvents: number;
  recentEvents: Vst3ComponentHandlerEvent[];
}

export interface Vst3ComponentHandlerEvent {
  sequence: number;
  kind: Vst3ComponentHandlerEventKind;
  parameterId?: number;
  valueNormalized?: number;
  flags?: number;
  restartFlags?: Vst3RestartFlags;
  dirty?: boolean;
  editorName?: string;
}
