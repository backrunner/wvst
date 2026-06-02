# WVST Web Audio Device Routing

更新日期：2026-06-02

## 结论

WVST 当前架构中，Web 是音频设备承载方，Bridge Server / host worker 是本地 VST 处理方。Web 侧选择输入/输出硬件设备后，把 WebAudio graph 中的音频块通过 WVST stream 送入本地 VST worker；worker 不直接打开声卡设备。

这样做的原因：

- 浏览器已经拥有麦克风、扬声器权限模型和用户授权 UI。
- AudioWorklet 负责实时图，Bridge/worker 负责插件隔离和处理，职责边界清晰。
- 避免 Bridge Server 与浏览器同时打开声卡造成双时钟、回声、权限和设备占用问题。
- 后续如果需要本地独立模式，可在 `wvst-embed` 或 native app runtime 中单独引入 CPAL/CoreAudio/WASAPI/JACK 设备路由，不影响 Web SDK。

## Web 输入设备

Web 通过 `navigator.mediaDevices.enumerateDevices()` 获取 `audioinput` 列表，通过 `getUserMedia({ audio: { deviceId } })` 指定输入设备。SDK 暴露：

- `listWVSTAudioDevices()`
- `requestWVSTAudioInput({ deviceId, channelCount })`
- `createWVSTAudioInputSource(audioContext, options)`

典型图：

```text
selected microphone -> MediaStreamAudioSourceNode -> app/VST input node -> WVST AudioWorklet/SAB -> Bridge -> host worker -> VST3 process
```

## Web 输出设备

输出设备优先尝试 `AudioContext.setSinkId()`。如果浏览器不支持，SDK 提供 `MediaStreamAudioDestinationNode + HTMLMediaElement.setSinkId()` route：

```text
VST output node -> MediaStreamAudioDestinationNode -> HTMLAudioElement.setSinkId(outputDeviceId)
```

SDK 暴露：

- `selectWVSTAudioOutputDevice()`
- `setWVSTAudioContextOutputDevice(audioContext, { deviceId })`
- `setWVSTMediaElementOutputDevice(audioElement, { deviceId })`
- `createWVSTMediaElementOutputRoute(audioContext, { outputDeviceId })`
- `createWVSTAudioDeviceSession(options)`

如果输出选择 API 不可用，SDK 返回 `false` 或抛出明确错误，应用不应静默假装设备已切换。

## 高层 session graph

Web SDK 的 `createWVSTAudioDeviceSession()` 将以下组件串起来：

```text
input device -> MediaStreamAudioSourceNode -> WVST AudioWorklet/SAB
WVST AudioWorklet/SAB -> Bridge worker audio pump -> host worker/VST
WVST AudioWorklet/SAB -> MediaStreamAudioDestinationNode -> selected output device
```

它负责：

- 按 instance 的 `inputChannels` / `outputChannels` 创建 SAB buffers。
- 创建并配置 `wvst-loopback` AudioWorkletNode。
- 当 instance `inputChannels > 0` 时按 `inputDeviceId` 打开输入设备；0-input 音源 VST 不打开麦克风。
- 按 `outputDeviceId` 创建 media-element 输出 route。
- 启动 `WVSTBridgeWorkerClient.startAudioStream()`。
- 失败或停止时释放 media tracks、断开 graph、停止 bridge audio pump。

它不负责：

- 创建 VST instance。
- 调用 `instance.start` / `instance.stop`。
- 参数自动化、MIDI、stream restart。

## 与 VST 输入输出的关系

Web 指定的是物理音频设备；VST instance create 指定的是插件处理通道数：

- `inputChannels`
- `outputChannels`
- `sampleRate`
- `maxBlockFrames`

两者需要由应用协调。例如选择单声道麦克风时，可以用 `inputChannels: 1` 创建实例；选择双声道输出时，可以用 `outputChannels: 2`。Bridge 只验证 stream/channel 约束，不负责选择硬件设备。

## 后续缺口

- 输出设备选择在不同浏览器中能力不一致，需要 capability API 和示例说明。
- 真实端到端设备切换需要测量 device change、sample rate change、stream restart 和 underflow/overflow 行为。
