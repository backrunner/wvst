import type { JsonValue } from "../client/transport.js";
import type { MidiEvent, ParameterAutomationEvent } from "../protocol/index.js";
import type { StreamLifecycleOptions } from "./instance-control.js";

export interface StreamSharedMemoryCreateOptions extends StreamLifecycleOptions {
  capacityBlocks?: number;
}

export interface StreamSharedMemoryDescriptor {
  schemaVersion: number;
  transport: "file-backed-mmap" | string;
  instanceId: number;
  streamId: number;
  path: string;
  totalBytes: number;
  descriptorBytes: number[];
  layout: SharedAudioTransportLayout;
  worker?: JsonValue;
}

export interface SharedAudioTransportConfig {
  sampleRateHz: number;
  blockFrames: number;
  capacityBlocks: number;
  inputChannels: number;
  outputChannels: number;
}

export interface SharedAudioTransportLayout {
  config: SharedAudioTransportConfig;
  input: SharedAudioRingLayout;
  output: SharedAudioRingLayout;
  totalBytes: number;
}

export interface SharedAudioRingLayout {
  role: "input" | "output";
  blockFrames: number;
  capacityBlocks: number;
  channels: number;
  capacityFrames: number;
  capacitySamples: number;
  cursorOffset: number;
  cursorBytes: number;
  audioOffset: number;
  audioBytes: number;
}

export interface SharedAudioRingCursorState {
  readFrame: number;
  writeFrame: number;
  droppedFrames: number;
  underrunFrames: number;
  overrunFrames: number;
  generation: number;
  flags: number;
}

export interface StreamSharedMemoryRingStatus {
  cursor: SharedAudioRingCursorState;
  readableFrames: number;
  writableFrames: number;
}

export interface StreamSharedMemoryStatus {
  descriptor: StreamSharedMemoryDescriptor;
  pathExists: boolean;
  input: StreamSharedMemoryRingStatus;
  output: StreamSharedMemoryRingStatus;
}

export interface StreamSharedMemoryError {
  code: number;
  message: string;
  data?: JsonValue;
}

export type StreamSharedMemoryCreateResult = StreamSharedMemoryDescriptor;

export interface StreamSharedMemoryDestroyResult {
  destroyed: boolean;
  descriptor: StreamSharedMemoryDescriptor | null;
  worker: StreamSharedMemoryWorkerDetachResult;
}

export type StreamSharedMemoryWorkerDetachResult =
  | { ok: true; result: JsonValue }
  | { ok: false; code: number; message: string; data?: JsonValue };

export interface StreamSharedMemoryStatusResult {
  attached: boolean;
  status: StreamSharedMemoryStatus | null;
  error?: StreamSharedMemoryError;
}

export interface StreamSharedMemoryProcessOptions extends StreamLifecycleOptions {
  frames?: number;
  midiEvents?: MidiEvent[];
  parameterEvents?: ParameterAutomationEvent[];
}

export interface StreamSharedMemoryProcessResult {
  instanceId: number;
  streamId: number;
  transport: "file-backed-mmap" | string;
  frames: number;
  inputEvents: StreamSharedMemoryInputEventReport;
  outputEventCount: number;
  advancedOutputEventCount: number;
  outputParameterChangeCount: number;
  [key: string]: JsonValue;
}

export interface StreamSharedMemoryInputEventReport {
  midiEvents: number;
  parameterEvents: number;
  vst3InputEvents: number;
  vst3ParameterChanges: number;
  [key: string]: JsonValue;
}

export interface StreamSharedMemoryPumpStartOptions extends StreamLifecycleOptions {
  frames?: number;
  intervalMicros?: number;
  adaptive?: boolean;
  minIntervalMicros?: number;
  maxIntervalMicros?: number;
  idleBackoffMicros?: number;
  maxQueuedEvents?: number;
}

export interface StreamSharedMemoryPumpConfig {
  instanceId: number;
  frames: number;
  intervalMicros: number;
  scheduling: StreamSharedMemoryPumpSchedulingConfig;
  maxQueuedEvents: number;
}

export type StreamSharedMemoryPumpSchedulingMode = "fixed" | "adaptive";

export interface StreamSharedMemoryPumpSchedulingConfig {
  mode: StreamSharedMemoryPumpSchedulingMode;
  baseIntervalMicros: number;
  minIntervalMicros: number;
  maxIntervalMicros: number;
  idleBackoffMicros: number;
  targetInputFrames: number;
  targetOutputHeadroomFrames: number;
}

export interface StreamSharedMemoryPumpRingStatus {
  inputChannels: number;
  outputChannels: number;
  inputReadableFrames: number;
  inputWritableFrames: number;
  outputReadableFrames: number;
  outputWritableFrames: number;
}

export interface StreamSharedMemoryPumpScheduleStatus {
  config: StreamSharedMemoryPumpSchedulingConfig;
  currentIntervalMicros: number;
  lastDelayMicros: number;
  lastRingStatus: StreamSharedMemoryPumpRingStatus | null;
}

export type StreamSharedMemoryPumpTickOutcome =
  | "success"
  | "input-underrun"
  | "output-backpressure"
  | "worker-error";

export interface StreamSharedMemoryPumpLastError {
  code: number;
  message: string;
  data?: JsonValue;
}

export type StreamSharedMemoryPumpEventOverflowPolicy =
  | "reject"
  | "drop-oldest";

export interface StreamSharedMemoryPumpEventQueueStatus {
  maxQueuedEvents: number;
  pendingBatches: number;
  pendingEvents: number;
  pendingMidiEvents: number;
  pendingParameterEvents: number;
  nextSequence: number;
  oldestTargetIteration: number | null;
  newestTargetIteration: number | null;
  enqueuedBatches: number;
  enqueuedEvents: number;
  drainedBatches: number;
  drainedEvents: number;
  lateBatches: number;
  lateEvents: number;
  droppedBatches: number;
  droppedEvents: number;
  clearedBatches: number;
  clearedEvents: number;
}

export interface StreamSharedMemoryPumpEnqueueEventsOptions
  extends StreamLifecycleOptions {
  targetIteration?: number;
  delayIterations?: number;
  midiEvents?: MidiEvent[];
  parameterEvents?: ParameterAutomationEvent[];
  overflowPolicy?: StreamSharedMemoryPumpEventOverflowPolicy;
}

export interface StreamSharedMemoryPumpEnqueueEventsResult {
  instanceId: number;
  targetIteration: number;
  sequence: number;
  queuedEvents: number;
  queuedMidiEvents: number;
  queuedParameterEvents: number;
  droppedEvents: number;
  droppedBatches: number;
  status: StreamSharedMemoryPumpEventQueueStatus;
}

export interface StreamSharedMemoryPumpClearEventsResult {
  instanceId: number;
  clearedEvents: number;
  clearedBatches: number;
  status: StreamSharedMemoryPumpEventQueueStatus;
}

export interface StreamSharedMemoryPumpStatus {
  running: boolean;
  taskFinished: boolean;
  uptimeMs: number;
  config: StreamSharedMemoryPumpConfig;
  iterations: number;
  successes: number;
  failures: number;
  preflightSkips: number;
  overruns: number;
  inputUnderruns: number;
  outputBackpressure: number;
  workerErrors: number;
  lastProcessMicros: number;
  maxProcessMicros: number;
  lastOverrunMicros: number | null;
  lastOutcome: StreamSharedMemoryPumpTickOutcome | null;
  lastSuccessFrames: number | null;
  schedule: StreamSharedMemoryPumpScheduleStatus;
  events: StreamSharedMemoryPumpEventQueueStatus;
  lastError: StreamSharedMemoryPumpLastError | null;
}

export interface StreamSharedMemoryPumpStartResult {
  started: boolean;
  status: StreamSharedMemoryPumpStatus;
}

export interface StreamSharedMemoryPumpStopResult {
  stopped: boolean;
  status: StreamSharedMemoryPumpStatus | null;
}

export interface StreamSharedMemoryPumpStatusResult {
  running: boolean;
  status: StreamSharedMemoryPumpStatus | null;
  error?: StreamSharedMemoryError;
}
