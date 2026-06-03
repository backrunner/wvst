export interface WVSTAudioDeviceList {
  inputs: MediaDeviceInfo[];
  outputs: MediaDeviceInfo[];
}

export interface WVSTAudioDeviceCapabilities {
  mediaDevices: boolean;
  enumerateDevices: boolean;
  getUserMedia: boolean;
  deviceChangeEvents: boolean;
  selectAudioOutput: boolean;
  mediaElementSinkId: boolean;
  audioContextSinkId: boolean;
  secureContext: boolean;
  crossOriginIsolated: boolean;
  sharedArrayBuffer: boolean;
}

export interface WVSTAudioDeviceWatcherOptions {
  immediate?: boolean;
  onChange(devices: WVSTAudioDeviceList): void | Promise<void>;
  onError?(error: Error): void;
}

export interface WVSTAudioDeviceWatcher {
  capabilities: WVSTAudioDeviceCapabilities;
  refresh(): Promise<WVSTAudioDeviceList>;
  stop(): void;
}

export interface WVSTAudioInputOptions {
  deviceId?: string;
  channelCount?: number;
  constraints?: MediaTrackConstraints;
}

export interface WVSTAudioInputSource {
  stream: MediaStream;
  node: MediaStreamAudioSourceNode;
  stop(): void;
}

export interface WVSTOutputDeviceOptions {
  deviceId: string;
}

export interface WVSTMediaElementOutputRouteOptions {
  outputDeviceId?: string;
  element?: HTMLMediaElement;
}

export interface WVSTMediaElementOutputRoute {
  destination: MediaStreamAudioDestinationNode;
  element: HTMLMediaElement;
  outputDeviceId?: string;
  setOutputDevice(deviceId: string): Promise<boolean>;
  start(): Promise<void>;
  stop(): void;
}

interface MediaDevicesWithAudioOutput extends MediaDevices {
  selectAudioOutput?: (options?: { deviceId?: string }) => Promise<MediaDeviceInfo>;
}

interface AudioContextWithSinkId extends AudioContext {
  setSinkId?: (sinkId: string) => Promise<void>;
}

export function getWVSTAudioDeviceCapabilities(
  context?: AudioContext,
): WVSTAudioDeviceCapabilities {
  const mediaDevices =
    typeof navigator === "object" ? navigator.mediaDevices : undefined;
  const mediaDevicesWithOutput = mediaDevices as MediaDevicesWithAudioOutput | undefined;

  return {
    mediaDevices: mediaDevices !== undefined,
    enumerateDevices: typeof mediaDevices?.enumerateDevices === "function",
    getUserMedia: typeof mediaDevices?.getUserMedia === "function",
    deviceChangeEvents:
      typeof mediaDevices?.addEventListener === "function" &&
      "ondevicechange" in (mediaDevices ?? {}),
    selectAudioOutput: typeof mediaDevicesWithOutput?.selectAudioOutput === "function",
    mediaElementSinkId:
      typeof HTMLMediaElement === "function" &&
      "setSinkId" in HTMLMediaElement.prototype,
    audioContextSinkId: hasAudioContextSinkId(context),
    secureContext:
      typeof globalThis.isSecureContext === "boolean"
        ? globalThis.isSecureContext
        : false,
    crossOriginIsolated:
      typeof globalThis.crossOriginIsolated === "boolean"
        ? globalThis.crossOriginIsolated
        : false,
    sharedArrayBuffer: typeof globalThis.SharedArrayBuffer === "function",
  };
}

export async function createWVSTAudioDeviceWatcher(
  options: WVSTAudioDeviceWatcherOptions,
): Promise<WVSTAudioDeviceWatcher> {
  const mediaDevices = requireMediaDevices();
  const capabilities = getWVSTAudioDeviceCapabilities();
  let stopped = false;

  const refresh = async (): Promise<WVSTAudioDeviceList> => listWVSTAudioDevices();
  const notify = async (): Promise<void> => {
    if (stopped) {
      return;
    }

    try {
      const devices = await refresh();
      if (!stopped) {
        await options.onChange(devices);
      }
    } catch (error) {
      options.onError?.(toError(error));
    }
  };
  const onDeviceChange = () => {
    void notify();
  };

  if (capabilities.deviceChangeEvents) {
    mediaDevices.addEventListener("devicechange", onDeviceChange);
  }
  if (options.immediate ?? true) {
    await notify();
  }

  return {
    capabilities,
    refresh,
    stop: () => {
      stopped = true;
      if (capabilities.deviceChangeEvents) {
        mediaDevices.removeEventListener("devicechange", onDeviceChange);
      }
    },
  };
}

export async function listWVSTAudioDevices(): Promise<WVSTAudioDeviceList> {
  const devices = await requireMediaDevices().enumerateDevices();

  return {
    inputs: devices.filter((device) => device.kind === "audioinput"),
    outputs: devices.filter((device) => device.kind === "audiooutput"),
  };
}

export async function requestWVSTAudioInput(
  options: WVSTAudioInputOptions = {},
): Promise<MediaStream> {
  const audio: MediaTrackConstraints = { ...(options.constraints ?? {}) };
  if (options.deviceId) {
    audio.deviceId = { exact: options.deviceId };
  }
  if (options.channelCount !== undefined) {
    audio.channelCount = options.channelCount;
  }

  return requireMediaDevices().getUserMedia({ audio });
}

export async function createWVSTAudioInputSource(
  context: AudioContext,
  options: WVSTAudioInputOptions = {},
): Promise<WVSTAudioInputSource> {
  const stream = await requestWVSTAudioInput(options);
  const node = context.createMediaStreamSource(stream);

  return {
    stream,
    node,
    stop: () => stopMediaStream(stream),
  };
}

export async function selectWVSTAudioOutputDevice(
  options: Partial<WVSTOutputDeviceOptions> = {},
): Promise<MediaDeviceInfo> {
  const devices = requireMediaDevices() as MediaDevicesWithAudioOutput;
  if (typeof devices.selectAudioOutput !== "function") {
    throw new Error("WVST audio output selection requires mediaDevices.selectAudioOutput");
  }

  return devices.selectAudioOutput(
    options.deviceId ? { deviceId: options.deviceId } : undefined,
  );
}

export async function setWVSTAudioContextOutputDevice(
  context: AudioContext,
  options: WVSTOutputDeviceOptions,
): Promise<boolean> {
  const contextWithSink = context as AudioContextWithSinkId;
  if (typeof contextWithSink.setSinkId !== "function") {
    return false;
  }

  await contextWithSink.setSinkId(options.deviceId);
  return true;
}

export async function setWVSTMediaElementOutputDevice(
  element: HTMLMediaElement,
  options: WVSTOutputDeviceOptions,
): Promise<boolean> {
  const setSinkId = (element as { setSinkId?: (sinkId: string) => Promise<void> })
    .setSinkId;
  if (typeof setSinkId !== "function") {
    return false;
  }

  await setSinkId.call(element, options.deviceId);
  return true;
}

export async function createWVSTMediaElementOutputRoute(
  context: AudioContext,
  options: WVSTMediaElementOutputRouteOptions = {},
): Promise<WVSTMediaElementOutputRoute> {
  const destination = context.createMediaStreamDestination();
  const element = options.element ?? new Audio();
  let outputDeviceId = options.outputDeviceId;
  element.srcObject = destination.stream;

  if (outputDeviceId) {
    const selected = await setWVSTMediaElementOutputDevice(element, {
      deviceId: outputDeviceId,
    });
    if (!selected) {
      outputDeviceId = undefined;
    }
  }

  const route: WVSTMediaElementOutputRoute = {
    destination,
    element,
    outputDeviceId,
    setOutputDevice: async (deviceId: string) => {
      const selected = await setWVSTMediaElementOutputDevice(element, { deviceId });
      if (selected) {
        route.outputDeviceId = deviceId;
      }
      return selected;
    },
    start: () => element.play(),
    stop: () => {
      element.pause();
      element.srcObject = null;
      stopMediaStream(destination.stream);
    },
  };

  return route;
}

function stopMediaStream(stream: MediaStream): void {
  for (const track of stream.getTracks()) {
    track.stop();
  }
}

function requireMediaDevices(): MediaDevices {
  if (typeof navigator !== "object" || !navigator.mediaDevices) {
    throw new Error("WVST audio device selection requires navigator.mediaDevices");
  }

  return navigator.mediaDevices;
}

function hasAudioContextSinkId(context: AudioContext | undefined): boolean {
  if (context) {
    return typeof (context as AudioContextWithSinkId).setSinkId === "function";
  }

  return typeof AudioContext === "function" && "setSinkId" in AudioContext.prototype;
}

function toError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}
