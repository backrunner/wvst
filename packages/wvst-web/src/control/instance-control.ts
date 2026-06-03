import type {
  BridgeEvent,
  JsonValue,
  Vst3ComponentHandlerEventKind,
  Vst3RestartFlags,
} from "../client/transport.js";
import type {
  StreamSharedMemoryPumpStatusResult,
  StreamSharedMemoryStatusResult,
} from "./shared-memory.js";

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
  state:
    | "allocated"
    | "starting"
    | "ready"
    | "processing"
    | "stopping"
    | "stopped"
    | "recovering"
    | "failed";
  workerState:
    | "not-started"
    | "starting"
    | "ready"
    | "processing"
    | "stopping"
    | "stopped"
    | "recovering"
    | "failed";
  streamState: "open" | "closed";
  backend?: string;
  controllerClassId?: string;
  runtimeCapabilities: RuntimeCapabilities;
  latencySamples: number;
  tailSamples: number;
  tailInfo: RuntimeTailInfo;
}

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

export interface InstanceStatusOptions {
  instanceId: number;
}

export type InstanceUnitsOptions = InstanceStatusOptions;

export type Vst3AudioBusDirection = "input" | "output";

export interface InstanceParametersOptions {
  instanceId: number;
}

export interface Vst3ParameterInfo {
  id: number;
  title: string | null;
  shortTitle: string | null;
  units: string | null;
  stepCount: number;
  defaultNormalizedValue: number;
  unitId: number;
  flags: Vst3ParameterFlags;
}

export interface Vst3ParameterFlags {
  raw: number;
  canAutomate: boolean;
  readOnly: boolean;
  wrapAround: boolean;
  list: boolean;
  hidden: boolean;
  programChange: boolean;
  bypass: boolean;
}

export interface InstanceParametersResult {
  instanceId: number;
  parameters: Vst3ParameterInfo[];
}

export interface InstanceParameterGetOptions {
  instanceId: number;
  parameterId: number;
}

export interface InstanceParameterGetResult {
  instanceId: number;
  parameterId: number;
  valueNormalized: number;
}

export interface InstanceParameterInfoOptions extends InstanceParameterGetOptions {
  valueNormalized?: number;
}

export interface InstanceParameterInfoResult extends InstanceParameterGetResult {
  valuePlain: number | null;
  valueString: string | null;
}

export interface InstanceParameterValueByStringOptions extends InstanceParameterGetOptions {
  value: string;
}

export type InstanceParameterValueByStringResult = InstanceParameterInfoResult;

export interface InstanceParameterNormalizedByPlainOptions extends InstanceParameterGetOptions {
  valuePlain: number;
}

export type InstanceParameterNormalizedByPlainResult = InstanceParameterInfoResult;

export interface InstanceParameterSetOptions extends InstanceParameterGetOptions {
  valueNormalized: number;
}

export type InstanceParameterSetResult = InstanceParameterGetResult;

export type InstanceParameterBeginEditOptions = InstanceParameterGetOptions;

export interface InstanceParameterEditResult {
  instanceId: number;
  parameterId: number;
  editKind: "begin-edit" | "perform-edit" | "end-edit";
  valueNormalized: number | null;
}

export interface InstanceParameterPerformEditOptions extends InstanceParameterGetOptions {
  valueNormalized: number;
}

export type InstanceParameterBeginEditResult = InstanceParameterEditResult;
export type InstanceParameterPerformEditResult = InstanceParameterEditResult;
export type InstanceParameterEndEditOptions = InstanceParameterGetOptions;
export type InstanceParameterEndEditResult = InstanceParameterEditResult;

export interface Vst3UnitMetadata {
  units: Vst3UnitInfoEntry[];
  programLists: Vst3ProgramList[];
  selectedUnitId: number;
}

export interface Vst3UnitInfoEntry {
  id: number;
  parentUnitId: number;
  name: string | null;
  programListId: number | null;
}

export interface Vst3ProgramList {
  id: number;
  name: string | null;
  programCount: number;
  programs: Vst3ProgramInfo[];
}

export interface Vst3ProgramInfo {
  index: number;
  name: string | null;
}

export interface InstanceUnitsResult {
  instanceId: number;
  unitInfo: Vst3UnitMetadata | null;
}

export interface InstanceSelectUnitOptions {
  instanceId: number;
  unitId: number;
}

export interface InstanceSelectUnitResult {
  instanceId: number;
  unitId: number;
  selectedUnitId: number;
}

export interface InstanceUnitByBusOptions {
  instanceId: number;
  direction: Vst3AudioBusDirection;
  busIndex: number;
  channel: number;
}

export interface InstanceUnitByBusResult extends InstanceUnitByBusOptions {
  unitId: number | null;
}

export interface InstanceSetUnitProgramDataOptions {
  instanceId: number;
  listOrUnitId: number;
  programIndex: number;
  dataBase64: string;
}

export interface InstanceSetUnitProgramDataResult {
  instanceId: number;
  listOrUnitId: number;
  programIndex: number;
  dataBytes: number;
}

export interface InstanceProgramDataOptions {
  instanceId: number;
  listId: number;
  programIndex: number;
}

export interface InstanceProgramDataSupportedResult extends InstanceProgramDataOptions {
  supported: boolean;
}

export interface InstanceProgramDataResult extends InstanceProgramDataOptions {
  dataBase64: string;
  dataBytes: number;
}

export interface InstanceSetProgramDataOptions extends InstanceProgramDataOptions {
  dataBase64: string;
}

export interface InstanceSetProgramDataResult extends InstanceProgramDataOptions {
  dataBytes: number;
}

export interface InstanceUnitDataOptions {
  instanceId: number;
  unitId: number;
}

export interface InstanceUnitDataSupportedResult extends InstanceUnitDataOptions {
  supported: boolean;
}

export interface InstanceUnitDataResult extends InstanceUnitDataOptions {
  dataBase64: string;
  dataBytes: number;
}

export interface InstanceSetUnitDataOptions extends InstanceUnitDataOptions {
  dataBase64: string;
}

export interface InstanceSetUnitDataResult extends InstanceUnitDataOptions {
  dataBytes: number;
}

export interface InstanceGetStateOptions {
  instanceId: number;
}

export interface InstanceGetStateResult {
  instanceId: number;
  componentStateBase64: string | null;
  controllerStateBase64: string | null;
}

export interface InstanceSetStateOptions {
  instanceId: number;
  componentStateBase64?: string;
  controllerStateBase64?: string;
}

export interface InstanceSetStateResult {
  instanceId: number;
  componentStateBytes: number | null;
  controllerStateBytes: number | null;
}

export interface InstanceConnectionNotifyOptions {
  instanceId: number;
  messageId: string;
  attributes?: Record<string, Vst3MessageAttribute>;
}

export interface InstanceConnectionNotifyResult extends InstanceConnectionNotifyOptions {
  target: "component" | "controller";
  notified: boolean;
  attributeCount: number;
}

export type Vst3MessageAttribute =
  | { type: "int"; value: number }
  | { type: "float"; value: number }
  | { type: "string"; value: string }
  | { type: "binary"; valueBase64: string };

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

export interface InstanceStatusResult {
  instance: InstanceDescriptor;
  worker: InstanceWorkerMetrics;
  recovered?: boolean;
}

export interface InstanceMetadataRefreshOptions extends InstanceStatusOptions {
  includeState?: boolean;
  includeWorkerMetrics?: boolean;
}

export interface InstanceMetadataRefreshResult {
  instanceId: number;
  parameters: Vst3ParameterInfo[] | null;
  unitInfo: Vst3UnitMetadata | null;
  state: InstanceGetStateResult | null;
  worker: InstanceWorkerMetrics | null;
}

export interface InstanceRuntimeSnapshotOptions extends InstanceMetadataRefreshOptions {
  includeRecentEvents?: boolean;
  afterEventSequence?: number;
}

export interface InstanceRuntimeSnapshotResult {
  instance: InstanceDescriptor;
  metadata: InstanceMetadataRefreshResult;
  dataPlane: InstanceDataPlaneSnapshot;
  recentEvents: BridgeEvent[] | null;
}

export interface InstanceDataPlaneSnapshot {
  sharedMemory: StreamSharedMemoryStatusResult;
  sharedMemoryPump: StreamSharedMemoryPumpStatusResult;
}

export interface InstanceRestartOptions {
  instanceId: number;
}

export type InstanceRestartResult = InstanceStatusResult;

export interface InstanceProcessingOptions {
  instanceId: number;
}

export type InstanceProcessingResult = InstanceStatusResult;

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

export interface InstanceParameterEditOptions extends InstanceParameterSetOptions {}

export interface InstanceParameterEditAggregateResult extends InstanceParameterSetResult {
  beginEdit: InstanceParameterBeginEditResult | null;
  performEdit: InstanceParameterPerformEditResult | null;
  endEdit: InstanceParameterEndEditResult | null;
}

export interface InstanceRefreshRequestOptions {
  includeState?: boolean;
  includeWorkerMetrics?: boolean;
}

export interface InstanceSetStateAndRefreshOptions
  extends InstanceSetStateOptions,
    InstanceRefreshRequestOptions {}

export interface InstanceSetStateAndRefreshResult {
  instanceId: number;
  setState: InstanceSetStateResult | null;
  metadata: InstanceMetadataRefreshResult;
}

export interface InstanceSetUnitProgramDataAndRefreshOptions
  extends InstanceSetUnitProgramDataOptions,
    InstanceRefreshRequestOptions {}

export interface InstanceSetUnitProgramDataAndRefreshResult {
  instanceId: number;
  setUnitProgramData: InstanceSetUnitProgramDataResult | null;
  metadata: InstanceMetadataRefreshResult;
}

export interface InstanceSetProgramDataAndRefreshOptions
  extends InstanceSetProgramDataOptions,
    InstanceRefreshRequestOptions {}

export interface InstanceSetProgramDataAndRefreshResult {
  instanceId: number;
  setProgramData: InstanceSetProgramDataResult | null;
  metadata: InstanceMetadataRefreshResult;
}

export interface InstanceSetUnitDataAndRefreshOptions
  extends InstanceSetUnitDataOptions,
    InstanceRefreshRequestOptions {}

export interface InstanceSetUnitDataAndRefreshResult {
  instanceId: number;
  setUnitData: InstanceSetUnitDataResult | null;
  metadata: InstanceMetadataRefreshResult;
}

export interface InstanceConnectionNotifyAndRefreshOptions
  extends InstanceConnectionNotifyOptions,
    InstanceRefreshRequestOptions {}

export interface InstanceConnectionNotifyAndRefreshResult {
  instanceId: number;
  notifyComponent?: InstanceConnectionNotifyResult | null;
  notifyController?: InstanceConnectionNotifyResult | null;
  metadata: InstanceMetadataRefreshResult;
}
