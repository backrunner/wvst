import {
  createWVSTAudioInputSource,
  createWVSTMediaElementOutputRoute,
  type WVSTAudioInputSource,
  type WVSTMediaElementOutputRoute,
} from "./devices.js";
import type { InstanceDescriptor } from "./instances.js";
import {
  createLoopbackAudioWorkletNode,
  createLoopbackSharedBuffers,
  type LoopbackSharedBuffers,
} from "./loopback.js";
import type { WVSTBridgeWorkerClient } from "./worker-client.js";

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
}

export interface WVSTAudioInputSwitchOptions {
  inputDeviceId?: string;
  inputConstraints?: MediaTrackConstraints;
}

export interface WVSTAudioDeviceSession {
  instance: InstanceDescriptor;
  buffers: LoopbackSharedBuffers;
  workletNode: AudioWorkletNode;
  input?: WVSTAudioInputSource;
  output: WVSTMediaElementOutputRoute;
  setInputDevice(options?: WVSTAudioInputSwitchOptions): Promise<void>;
  setOutputDevice(deviceId: string): Promise<boolean>;
  startOutput(): Promise<void>;
  stop(): Promise<void>;
}

export async function createWVSTAudioDeviceSession(
  options: WVSTAudioDeviceSessionOptions,
): Promise<WVSTAudioDeviceSession> {
  const frames = options.frames ?? 128;
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
    await options.bridgeWorker.startAudioStream({
      streamId: options.instance.streamId,
      sampleRate: options.instance.sampleRate,
      frames,
      inputChannels: options.instance.inputChannels,
      outputChannels: options.instance.outputChannels,
      buffers,
    });
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

function createSession(
  options: WVSTAudioDeviceSessionOptions,
  buffers: LoopbackSharedBuffers,
  workletNode: AudioWorkletNode,
  input: WVSTAudioInputSource | undefined,
  output: WVSTMediaElementOutputRoute,
): WVSTAudioDeviceSession {
  let stopped = false;
  let currentInput = input;

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
      try {
        await options.bridgeWorker.stopAudioStream(options.instance.streamId);
      } finally {
        cleanupGraph(workletNode, currentInput, output);
        currentInput = undefined;
        session.input = undefined;
      }
    },
  };

  return session;
}
