export {
  WVSTClient,
  type ConnectOptions,
  type HelloRequest,
  type LowLatencyPrerequisites,
} from "./client.js";
export {
  AUDIO_FRAME_HEADER_BYTES,
  AUDIO_FRAME_MAGIC,
  AUDIO_FRAME_VERSION,
  AudioSampleFormat,
  AudioFrameFlags,
  decodeAudioFrameHeader,
  encodeAudioFrameHeader,
  type AudioFrameHeader,
} from "./protocol.js";

