import {
  readLoopbackMetrics,
  type LoopbackSharedBuffers,
} from "../audio/loopback.js";

export type WVSTAudioStreamRestartReason = "manual" | "transport-failure";

export interface WVSTAudioStreamRestartEvent {
  reason: WVSTAudioStreamRestartReason;
  restartCount: number;
  transportFailures: number;
  atMs: number;
}

export interface WVSTAudioStreamRestartFailureEvent {
  reason: WVSTAudioStreamRestartReason;
  failureCount: number;
  transportFailures: number;
  error: string;
  atMs: number;
}

export interface WVSTAudioGraphRebuildRequiredEvent {
  reason: "sample-rate-mismatch";
  contextSampleRate: number;
  instanceSampleRate: number;
  atMs: number;
}

export interface WVSTAudioStreamAutoRestartOptions {
  monitorIntervalMs?: number;
  minRestartIntervalMs?: number;
  maxRestartAttempts?: number;
  onRestart?: (event: WVSTAudioStreamRestartEvent) => void;
  onRestartFailure?: (event: WVSTAudioStreamRestartFailureEvent) => void;
  onRebuildRequired?: (event: WVSTAudioGraphRebuildRequiredEvent) => void;
}

export interface WVSTAudioDeviceSessionRecoveryMetrics {
  audioStreamRestarts: number;
  audioStreamRestartFailures: number;
  audioStreamAutoRestartAttempts: number;
  audioStreamRestartInFlight: boolean;
  audioStreamAutoRestartEnabled: boolean;
  audioStreamAutoRestartExhausted: boolean;
  rebuildRequired: boolean;
  sampleRateMismatches: number;
  contextSampleRate: number;
  instanceSampleRate: number;
  lastAudioStreamRestartReason?: WVSTAudioStreamRestartReason;
  lastAudioStreamRestartAtMs?: number;
  lastAudioStreamRestartFailure?: string;
  lastRebuildRequiredAtMs?: number;
}

export interface WVSTAudioSessionRecoveryOptions {
  context: AudioContext;
  instanceSampleRate: number;
  buffers: LoopbackSharedBuffers;
  autoRestartAudioStream?: boolean | WVSTAudioStreamAutoRestartOptions;
  isStopped: () => boolean;
  restartStream: (reason: WVSTAudioStreamRestartReason) => Promise<boolean>;
}

export interface WVSTAudioSessionRecoveryController {
  start(): void;
  stop(): void;
  restartManually(): Promise<void>;
  assertSampleRateUsable(): void;
  metrics(): WVSTAudioDeviceSessionRecoveryMetrics;
}

interface NormalizedAutoRestartOptions {
  enabled: boolean;
  monitorIntervalMs: number;
  minRestartIntervalMs: number;
  maxRestartAttempts: number;
  callbacks?: WVSTAudioStreamAutoRestartOptions;
}

const DEFAULT_MONITOR_INTERVAL_MS = 250;
const DEFAULT_MIN_RESTART_INTERVAL_MS = 750;
const DEFAULT_MAX_RESTART_ATTEMPTS = 8;

export function createWVSTAudioSessionRecovery(
  options: WVSTAudioSessionRecoveryOptions,
): WVSTAudioSessionRecoveryController {
  const autoRestart = normalizeAutoRestartOptions(options.autoRestartAudioStream);
  let monitorTimer: number | undefined;
  let lastTransportFailures = readLoopbackMetrics(options.buffers).transportFailures;
  let lastAutoRestartAttemptAtMs = 0;
  let automaticRestartAttempts = 0;
  let autoRestartExhausted = false;
  let audioStreamRestarts = 0;
  let audioStreamRestartFailures = 0;
  let restartInFlight = false;
  let restartPromise: Promise<void> | undefined;
  let rebuildRequired = false;
  let sampleRateMismatches = 0;
  let lastMismatchSampleRate: number | undefined;
  let lastAudioStreamRestartReason: WVSTAudioStreamRestartReason | undefined;
  let lastAudioStreamRestartAtMs: number | undefined;
  let lastAudioStreamRestartFailure: string | undefined;
  let lastRebuildRequiredAtMs: number | undefined;

  return {
    start,
    stop,
    restartManually,
    assertSampleRateUsable,
    metrics,
  };

  function start(): void {
    if (!autoRestart.enabled) {
      return;
    }

    monitorTimer = globalThis.setInterval(
      checkSessionHealth,
      autoRestart.monitorIntervalMs,
    );
  }

  function stop(): void {
    if (monitorTimer !== undefined) {
      globalThis.clearInterval(monitorTimer);
      monitorTimer = undefined;
    }
  }

  function restartManually(): Promise<void> {
    automaticRestartAttempts = 0;
    autoRestartExhausted = false;
    return restartAudioStream("manual");
  }

  function assertSampleRateUsable(): void {
    const contextSampleRate = currentContextSampleRate(options.context);
    if (contextSampleRate === options.instanceSampleRate) {
      rebuildRequired = false;
      lastMismatchSampleRate = undefined;
      return;
    }

    noteRebuildRequired(contextSampleRate);
    throw new Error(
      `WVST audio graph rebuild required: AudioContext is ${contextSampleRate} Hz, instance is ${options.instanceSampleRate} Hz`,
    );
  }

  function metrics(): WVSTAudioDeviceSessionRecoveryMetrics {
    return {
      audioStreamRestarts,
      audioStreamRestartFailures,
      audioStreamAutoRestartAttempts: automaticRestartAttempts,
      audioStreamRestartInFlight: restartInFlight,
      audioStreamAutoRestartEnabled: autoRestart.enabled,
      audioStreamAutoRestartExhausted: autoRestartExhausted,
      rebuildRequired,
      sampleRateMismatches,
      contextSampleRate: currentContextSampleRate(options.context),
      instanceSampleRate: options.instanceSampleRate,
      lastAudioStreamRestartReason,
      lastAudioStreamRestartAtMs,
      lastAudioStreamRestartFailure,
      lastRebuildRequiredAtMs,
    };
  }

  function checkSessionHealth(): void {
    if (options.isStopped()) {
      return;
    }
    try {
      assertSampleRateUsable();
    } catch {
      return;
    }

    const currentMetrics = readLoopbackMetrics(options.buffers);
    if (currentMetrics.transportFailures <= lastTransportFailures) {
      return;
    }
    maybeAutoRestart(currentMetrics.transportFailures);
  }

  function noteRebuildRequired(contextSampleRate: number): void {
    rebuildRequired = true;
    if (lastMismatchSampleRate === contextSampleRate) {
      return;
    }

    lastMismatchSampleRate = contextSampleRate;
    sampleRateMismatches += 1;
    lastRebuildRequiredAtMs = Date.now();
    autoRestart.callbacks?.onRebuildRequired?.({
      reason: "sample-rate-mismatch",
      contextSampleRate,
      instanceSampleRate: options.instanceSampleRate,
      atMs: lastRebuildRequiredAtMs,
    });
  }

  function maybeAutoRestart(transportFailures: number): void {
    if (restartInFlight || rebuildRequired) {
      return;
    }
    if (automaticRestartAttempts >= autoRestart.maxRestartAttempts) {
      autoRestartExhausted = true;
      lastTransportFailures = transportFailures;
      return;
    }

    const now = Date.now();
    if (now - lastAutoRestartAttemptAtMs < autoRestart.minRestartIntervalMs) {
      return;
    }

    automaticRestartAttempts += 1;
    lastAutoRestartAttemptAtMs = now;
    void restartAudioStream("transport-failure")
      .then(() => {
        lastTransportFailures = readLoopbackMetrics(options.buffers).transportFailures;
      })
      .catch(() => {
        if (automaticRestartAttempts >= autoRestart.maxRestartAttempts) {
          autoRestartExhausted = true;
          lastTransportFailures = readLoopbackMetrics(
            options.buffers,
          ).transportFailures;
        }
      });
  }

  function restartAudioStream(
    reason: WVSTAudioStreamRestartReason,
  ): Promise<void> {
    if (restartPromise) {
      return restartPromise;
    }

    restartInFlight = true;
    restartPromise = restartAudioStreamInner(reason)
      .then((restarted) => {
        if (!restarted) {
          return;
        }
        audioStreamRestarts += 1;
        lastAudioStreamRestartReason = reason;
        lastAudioStreamRestartAtMs = Date.now();
        lastAudioStreamRestartFailure = undefined;
        autoRestart.callbacks?.onRestart?.({
          reason,
          restartCount: audioStreamRestarts,
          transportFailures: readLoopbackMetrics(options.buffers).transportFailures,
          atMs: lastAudioStreamRestartAtMs,
        });
      })
      .catch((error: unknown) => {
        audioStreamRestartFailures += 1;
        lastAudioStreamRestartReason = reason;
        lastAudioStreamRestartFailure =
          error instanceof Error ? error.message : String(error);
        autoRestart.callbacks?.onRestartFailure?.({
          reason,
          failureCount: audioStreamRestartFailures,
          transportFailures: readLoopbackMetrics(options.buffers).transportFailures,
          error: lastAudioStreamRestartFailure,
          atMs: Date.now(),
        });
        throw error;
      })
      .finally(() => {
        restartInFlight = false;
        restartPromise = undefined;
      });

    return restartPromise;
  }

  async function restartAudioStreamInner(
    reason: WVSTAudioStreamRestartReason,
  ): Promise<boolean> {
    assertSampleRateUsable();
    const restarted = await options.restartStream(reason);
    if (restarted && reason === "manual") {
      lastTransportFailures = readLoopbackMetrics(options.buffers).transportFailures;
    }
    return restarted;
  }
}

function currentContextSampleRate(context: AudioContext): number {
  return Math.round(context.sampleRate);
}

function normalizeAutoRestartOptions(
  config: boolean | WVSTAudioStreamAutoRestartOptions | undefined,
): NormalizedAutoRestartOptions {
  if (config === false) {
    return {
      enabled: false,
      monitorIntervalMs: DEFAULT_MONITOR_INTERVAL_MS,
      minRestartIntervalMs: DEFAULT_MIN_RESTART_INTERVAL_MS,
      maxRestartAttempts: DEFAULT_MAX_RESTART_ATTEMPTS,
    };
  }

  const callbacks = typeof config === "object" ? config : undefined;
  return {
    enabled: true,
    monitorIntervalMs: positiveIntegerOrDefault(
      "monitorIntervalMs",
      callbacks?.monitorIntervalMs,
      DEFAULT_MONITOR_INTERVAL_MS,
    ),
    minRestartIntervalMs: positiveIntegerOrDefault(
      "minRestartIntervalMs",
      callbacks?.minRestartIntervalMs,
      DEFAULT_MIN_RESTART_INTERVAL_MS,
    ),
    maxRestartAttempts: nonNegativeIntegerOrDefault(
      "maxRestartAttempts",
      callbacks?.maxRestartAttempts,
      DEFAULT_MAX_RESTART_ATTEMPTS,
    ),
    callbacks,
  };
}

function positiveIntegerOrDefault(
  name: string,
  value: number | undefined,
  fallback: number,
): number {
  if (value === undefined) {
    return fallback;
  }
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`WVST ${name} must be a positive integer`);
  }
  return value;
}

function nonNegativeIntegerOrDefault(
  name: string,
  value: number | undefined,
  fallback: number,
): number {
  if (value === undefined) {
    return fallback;
  }
  if (!Number.isInteger(value) || value < 0) {
    throw new Error(`WVST ${name} must be a non-negative integer`);
  }
  return value;
}
