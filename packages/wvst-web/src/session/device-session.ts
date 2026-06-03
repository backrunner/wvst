import {
  createWVSTAudioInputSource,
  createWVSTMediaElementOutputRoute,
  type WVSTAudioInputSource,
  type WVSTMediaElementOutputRoute,
} from "./devices.js";
import type { InstanceDescriptor } from "../control/instances.js";
import {
  createLoopbackAudioWorkletNode,
  createLoopbackSharedBuffers,
  readLoopbackMetrics,
  type LoopbackMetrics,
  type LoopbackSharedBuffers,
} from "../audio/loopback.js";
import type { MidiEvent, ParameterAutomationEvent } from "../protocol/index.js";
import {
  createWVSTAudioSessionRecovery,
  type WVSTAudioDeviceSessionRecoveryMetrics,
  type WVSTAudioStreamAutoRestartOptions,
} from "./recovery.js";
import type {
  BridgeWorkerAudioStreamOptions,
  WVSTBridgeWorkerClient,
} from "../client/worker-client.js";

export type {
  WVSTAudioGraphRebuildRequiredEvent,
  WVSTAudioStreamAutoRestartOptions,
  WVSTAudioStreamRestartEvent,
  WVSTAudioStreamRestartFailureEvent,
  WVSTAudioStreamRestartReason,
} from "./recovery.js";

export interface WVSTAudioDeviceSessionOptions {
  context: AudioContext;
  bridgeWorker: WVSTBridgeWorkerClient;
  instance: InstanceDescriptor;
  processorUrl: string;
  frames?: number;
  capacityQuanta?: number;
  inputDeviceId?: string;
  outputDeviceId?: string;
  inputConstraints?: MediaTrackConstraints;
  outputElement?: HTMLMediaElement;
  startOutput?: boolean;
  autoRestartAudioStream?: boolean | WVSTAudioStreamAutoRestartOptions;
}

export interface WVSTAudioInputSwitchOptions {
  inputDeviceId?: string;
  inputConstraints?: MediaTrackConstraints;
}

export interface WVSTAudioDeviceSessionMetrics
  extends LoopbackMetrics,
    WVSTAudioDeviceSessionRecoveryMetrics {}

export interface WVSTAudioDeviceSession {
  instance: InstanceDescriptor;
  buffers: LoopbackSharedBuffers;
  workletNode: AudioWorkletNode;
  input?: WVSTAudioInputSource;
  output: WVSTMediaElementOutputRoute;
  setInputDevice(options?: WVSTAudioInputSwitchOptions): Promise<void>;
  setOutputDevice(deviceId: string): Promise<boolean>;
  sendMidiEvents(events: MidiEvent[]): Promise<void>;
  sendParameterEvents(events: ParameterAutomationEvent[]): Promise<void>;
  restartAudioStream(): Promise<void>;
  getMetrics(): WVSTAudioDeviceSessionMetrics;
  startOutput(): Promise<void>;
  stop(): Promise<void>;
}

export async function createWVSTAudioDeviceSession(
  options: WVSTAudioDeviceSessionOptions,
): Promise<WVSTAudioDeviceSession> {
  const frames = options.frames ?? 128;
  assertSampleRateMatchesContext(options);
  const buffers = createLoopbackSharedBuffers({
    frames,
    inputChannels: options.instance.inputChannels,
    outputChannels: options.instance.outputChannels,
    capacityQuanta: options.capacityQuanta,
  });
  const workletNode = await createLoopbackAudioWorkletNode(options.context, {
    processorUrl: options.processorUrl,
    inputChannels: options.instance.inputChannels,
    outputChannels: options.instance.outputChannels,
    buffers,
  });
  let input: WVSTAudioInputSource | undefined;
  let output: WVSTMediaElementOutputRoute | undefined;

  try {
    input = await createInputSource(options);
    output = await createWVSTMediaElementOutputRoute(options.context, {
      outputDeviceId: options.outputDeviceId,
      element: options.outputElement,
    });
    input?.node.connect(workletNode);
    workletNode.connect(output.destination);
    await startBridgeAudioStream(options, buffers, frames);
  } catch (error) {
    cleanupGraph(workletNode, input, output);
    throw error;
  }

  const session = createSession(options, buffers, workletNode, input, output);
  if (options.startOutput) {
    try {
      await session.startOutput();
    } catch (error) {
      await session.stop();
      throw error;
    }
  }

  return session;
}

async function createInputSource(
  options: WVSTAudioDeviceSessionOptions,
): Promise<WVSTAudioInputSource | undefined> {
  if (options.instance.inputChannels === 0) {
    return undefined;
  }

  return createWVSTAudioInputSource(options.context, {
    deviceId: options.inputDeviceId,
    channelCount: options.instance.inputChannels,
    constraints: options.inputConstraints,
  });
}

function cleanupGraph(
  workletNode: AudioWorkletNode,
  input: WVSTAudioInputSource | undefined,
  output: WVSTMediaElementOutputRoute | undefined,
): void {
  input?.node.disconnect();
  workletNode.disconnect();
  input?.stop();
  output?.stop();
}

function startBridgeAudioStream(
  options: WVSTAudioDeviceSessionOptions,
  buffers: LoopbackSharedBuffers,
  frames: number,
): Promise<void> {
  return options.bridgeWorker.startAudioStream(
    streamOptions(options, buffers, frames),
  );
}

function streamOptions(
  options: WVSTAudioDeviceSessionOptions,
  buffers: LoopbackSharedBuffers,
  frames: number,
): BridgeWorkerAudioStreamOptions {
  return {
    streamId: options.instance.streamId,
    sampleRate: options.instance.sampleRate,
    frames,
    inputChannels: options.instance.inputChannels,
    outputChannels: options.instance.outputChannels,
    buffers,
  };
}

function assertSampleRateMatchesContext(options: WVSTAudioDeviceSessionOptions): void {
  const contextSampleRate = currentContextSampleRate(options);
  if (contextSampleRate !== options.instance.sampleRate) {
    throw new Error(
      `WVST session sample rate mismatch: AudioContext is ${contextSampleRate} Hz, instance is ${options.instance.sampleRate} Hz`,
    );
  }
}

function currentContextSampleRate(options: WVSTAudioDeviceSessionOptions): number {
  return Math.round(options.context.sampleRate);
}

function createSession(
  options: WVSTAudioDeviceSessionOptions,
  buffers: LoopbackSharedBuffers,
  workletNode: AudioWorkletNode,
  input: WVSTAudioInputSource | undefined,
  output: WVSTMediaElementOutputRoute,
): WVSTAudioDeviceSession {
  let stopped = false;
  let streamActive = true;
  let currentInput = input;
  const recovery = createWVSTAudioSessionRecovery({
    context: options.context,
    instanceSampleRate: options.instance.sampleRate,
    buffers,
    autoRestartAudioStream: options.autoRestartAudioStream,
    isStopped: () => stopped,
    restartStream,
  });

  const session: WVSTAudioDeviceSession = {
    instance: options.instance,
    buffers,
    workletNode,
    input: currentInput,
    output,
    setInputDevice: async (switchOptions: WVSTAudioInputSwitchOptions = {}) => {
      if (stopped) {
        throw new Error("WVST audio device session is stopped");
      }
      if (options.instance.inputChannels === 0) {
        throw new Error("WVST audio device session has no input bus");
      }

      const nextInput = await createWVSTAudioInputSource(options.context, {
        deviceId: switchOptions.inputDeviceId,
        channelCount: options.instance.inputChannels,
        constraints: switchOptions.inputConstraints,
      });

      try {
        nextInput.node.connect(workletNode);
      } catch (error) {
        nextInput.stop();
        throw error;
      }

      currentInput?.node.disconnect();
      currentInput?.stop();
      currentInput = nextInput;
      session.input = nextInput;
    },
    setOutputDevice: (deviceId: string) => {
      if (stopped) {
        throw new Error("WVST audio device session is stopped");
      }

      return output.setOutputDevice(deviceId);
    },
    sendMidiEvents: (events: MidiEvent[]) => {
      if (stopped) {
        throw new Error("WVST audio device session is stopped");
      }

      return options.bridgeWorker.sendMidiEvents({
        streamId: options.instance.streamId,
        events,
      });
    },
    sendParameterEvents: (events: ParameterAutomationEvent[]) => {
      if (stopped) {
        throw new Error("WVST audio device session is stopped");
      }

      return options.bridgeWorker.sendParameterEvents({
        streamId: options.instance.streamId,
        events,
      });
    },
    restartAudioStream: () => {
      if (stopped) {
        throw new Error("WVST audio device session is stopped");
      }

      return recovery.restartManually();
    },
    getMetrics: () => ({
      ...readLoopbackMetrics(buffers),
      ...recovery.metrics(),
    }),
    startOutput: () => {
      if (stopped) {
        throw new Error("WVST audio device session is stopped");
      }

      return output.start();
    },
    stop: async () => {
      if (stopped) {
        return;
      }
      stopped = true;
      recovery.stop();
      try {
        if (streamActive) {
          await options.bridgeWorker.stopAudioStream(options.instance.streamId);
          streamActive = false;
        }
      } finally {
        cleanupGraph(workletNode, currentInput, output);
        currentInput = undefined;
        session.input = undefined;
      }
    },
  };

  recovery.start();
  return session;

  async function restartStream(): Promise<boolean> {
    if (streamActive) {
      await options.bridgeWorker.stopAudioStream(options.instance.streamId);
      streamActive = false;
    }
    if (stopped) {
      return false;
    }
    await startBridgeAudioStream(options, buffers, buffers.frames);
    streamActive = true;
    return true;
  }
}
