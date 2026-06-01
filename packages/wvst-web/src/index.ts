export {
  WVSTClient,
  type BridgeHelloResult,
  type ConnectOptions,
  type HelloParams,
  type HelloRequest,
  type LowLatencyPrerequisites,
  type ProtocolVersion,
} from "./client.js";
export {
  WebSocketRpcTransport,
  WVSTBridgeError,
  type BridgeMetrics,
  type JsonValue,
  type RpcTransport,
} from "./transport.js";
export {
  WVSTBridgeWorkerClient,
  type BridgeWorkerAudioStreamOptions,
  type BridgeWorkerClientOptions,
} from "./worker-client.js";
export {
  LoopbackCounter,
  configureLoopbackAudioWorkletNode,
  createLoopbackAudioWorkletNode,
  createLoopbackSharedBuffers,
  readLoopbackOutput,
  writeLoopbackInput,
  type LoopbackBufferOptions,
  type LoopbackNodeOptions,
  type LoopbackSharedBuffers,
} from "./loopback.js";
export {
  type InstanceApi,
  type InstanceCreateOptions,
  type InstanceDescriptor,
  type InstanceDestroyOptions,
  type InstanceDestroyResult,
  type InstanceProcessingOptions,
  type InstanceProcessingResult,
  type InstanceRestartOptions,
  type InstanceRestartResult,
  type InstanceStatusOptions,
  type InstanceStatusResult,
  type InstanceWorkerMetrics,
  type StreamLifecycleOptions,
} from "./instances.js";
export {
  type PluginApi,
  type PluginClass,
  type PluginDescriptor,
  type PluginFactoryClass,
  type PluginFactoryInfo,
  type PluginFactoryInfoOptions,
  type PluginListOptions,
  type PluginScanFailure,
  type PluginScanOptions,
  type PluginScanReport,
} from "./plugins.js";
export {
  AUDIO_FRAME_HEADER_BYTES,
  AUDIO_FRAME_MAGIC,
  AUDIO_FRAME_VERSION,
  AudioSampleFormat,
  AudioFrameFlags,
  decodeAudioFrame,
  decodeAudioFrameHeader,
  encodeAudioFrame,
  encodeAudioFrameHeader,
  type AudioFrameHeader,
  type DecodedAudioFrame,
} from "./protocol.js";
