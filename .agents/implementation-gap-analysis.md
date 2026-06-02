# WVST Implementation Gap Analysis

更新日期：2026-06-02

## 当前已完成能力

- Rust workspace、TS/Rollup Web SDK、协议帧和基础测试已建立。
- Bridge Server 已提供 localhost WebSocket、hello/version negotiation、origin/token 校验、metrics 和二进制帧 echo。
- Web SDK 已提供 `WVSTClient.connect()`、低延迟前置条件检查、插件 scan/list API、worker/worklet loopback 基础模块。
- macOS VST3 scanner 已支持默认路径和自定义路径，能解析 `moduleinfo.json` 并回退到 bundle 名称。
- `wvst-host-worker` 已作为独立进程入口，支持 VST3 bundle 描述、module symbol probe、macOS factory info 读取和 passthrough probe。
- Bridge 已通过 `plugin.factoryInfo` 将 factory metadata 请求路由到隔离 worker，Bridge 自身不加载第三方 VST。
- Bridge 控制面已增加连接级会话门禁，除 `bridge.hello` 外的 API 需要先完成授权 hello。
- Bridge/Web SDK 已提供 `instance.create` / `instance.list` / `instance.destroy` 控制面，能为已扫描插件分配独立 `instanceId` 与 `streamId`。
- `wvst-host-worker serve` 已提供常驻控制 IPC，支持 worker hello 和 fake passthrough instance create/destroy；音频处理已从 debug JSON 小块处理迁移到独立二进制 audio IPC。
- Bridge `instance.create` 已能启动并绑定 `wvst-host-worker serve`，成功后实例进入 `ready` / `ready` 状态。
- Bridge worker supervisor 已加入 stderr 摘要、启动失败计数和基础 quarantine，避免同一故障插件无限重启。
- Bridge/Web SDK 已提供 `instance.status` heartbeat API，能通过 worker `worker.metrics` 检查实例 worker 存活，并在 worker 退出或 IPC 断开时把实例标记为 `failed`。
- Bridge/Web SDK 已提供 `instance.restart` 手动恢复 API，能在保留 `instanceId` / `streamId` 的情况下杀掉旧 worker 并重新拉起同一实例；Bridge metrics 已暴露 `workerFailures` 和 `workerRestarts`。
- Bridge/Web SDK 已提供 `instance.start` / `instance.stop` 处理生命周期控制，实例状态可从 `ready` 切到 `processing` / `stopped`。
- Bridge 音频路由现在要求实例处于 `processing` 状态；未 start、已 stop 或处理失败都会返回带 `silence` / `process-error` 的诊断静音帧，而不是继续把音频送进 worker。
- Instance heartbeat 已避免把正在 `processing` 的实例误降回 `ready`，降低控制面状态刷新对数据面的干扰。
- Bridge worker supervisor 已校验 `worker.hello` 中的 `ipcVersion`、`instanceLifecycle` 和 `binaryAudioProcess` capability，避免 Bridge 与不兼容 worker 继续创建实例。
- Bridge 创建 worker 实例时已把 `sampleRate` 和 `maxBlockFrames` 传给 worker；worker audio IPC 会验证 sample rate、最大 block、输入通道数和 processing 状态。
- `wvst-host-worker` 已为每个实例预分配 audio scratch buffers，并复用 audio IPC request/response body buffers；passthrough audio IPC 不再为每个 block 重复分配输入/输出 sample Vec 或响应 frame Vec。
- Bridge 二进制音频帧已能按 `streamId` 路由到对应 worker 的独立二进制 audio IPC，并回传 worker 处理后的 F32 frame；未匹配实例或非法帧暂时保留 echo fallback。
- Bridge metrics 已区分二进制帧总量、成功路由音频帧、fallback echo 和音频路由失败，便于后续接入 drop/late/underflow/overflow 统计。
- Bridge/Web SDK 已提供 `stream.open` / `stream.close` 控制 API，实例记录包含 `streamState`，Bridge 只将 open stream 的音频帧路由到 worker。
- 对于已知 stream 的关闭或处理失败场景，Bridge 会返回带 `silence` / `end-of-stream` / `process-error` flag 的诊断静音音频帧，避免把异常伪装成正常 echo。
- Web `bridge-worker` 已具备从 SAB input ring 读取 quantum、编码 WVST binary audio frame、发送 Bridge 并写回 output ring 的基础 audio pump；AudioWorklet processor 已支持通过 SAB ring 和计数器交换音频块。
- Web SDK 已提供 `WVSTBridgeWorkerClient`，封装 bridge worker 的 connect/request/sendBinary/startAudioStream/stopAudioStream 命令，避免应用侧手写 worker message protocol。
- Web SDK 已提供音频设备选择 helper：可枚举 `audioinput`/`audiooutput`，按 `deviceId` 请求输入 `MediaStream`，创建 `MediaStreamAudioSourceNode`，并通过 `AudioContext.setSinkId()` 或 `MediaStreamAudioDestinationNode + HTMLMediaElement.setSinkId()` 指定输出设备。
- Web SDK 已提供 `createWVSTAudioDeviceSession()` 高层 session graph helper，可按 Web 指定的输入/输出设备把 `MediaStreamAudioSourceNode`、WVST AudioWorklet/SAB、Bridge worker audio pump 和 media-element output route 串起来，并支持 0-input 音源 VST 的 Web 数据面启动。
- Web SDK 已提供音频设备 capability API、`devicechange` watcher，以及 session 运行期 `setInputDevice()` / `setOutputDevice()` 切换方法。
- Web session 已校验 `AudioContext.sampleRate` 与 instance `sampleRate` 一致，提供 `restartAudioStream()` 重启 Bridge worker audio pump，并通过 `getMetrics()` 暴露 loopback underflow/overflow 和 pending quantum 指标。
- `wvst-protocol` 和 Web SDK 已定义固定 16 字节 MIDI/note event schema，事件包含 sample offset、kind、channel、data bytes 和 note id；audio frame payload 已能表达 audio samples 后追加 event section。
- Web SDK 已提供 `sendMidiEvents()` 路径，DedicatedWorker 会缓存 MIDI events、按 sample offset 排序，并随下一块 audio frame 发送到 Bridge；Bridge/worker 音频 IPC 已能接受 event section，worker 会将 note on/off、poly pressure 和对应 raw MIDI note 事件转换为 VST3 `IEventList` 输入。
- Rust audio frame 协议已允许 `channels = 0`，passthrough worker 路径已支持 zero-input instrument frame 并覆盖测试。
- `wvst-vst3-host` 已增加 VST3 FUID 规范化、`IPluginFactory::createInstance` ABI skeleton 和 macOS `create_vst3_component_probe()` safe facade；`wvst-host-worker component-probe <plugin.vst3> <class-id>` 可在隔离 worker 内验证 component 创建并释放。
- VST3 ABI 边界已补入 `IPluginBase`、`IComponent`、`IAudioProcessor`、`ProcessSetup`、`AudioBusBuffers` 和 `ProcessData` 的 Rust repr(C) skeleton，后续真实 process path 可以继续在 `wvst-vst3-host` 内收敛 unsafe。
- `create_vst3_component_probe()` 现在会通过 `queryInterface` 验证 component 是否暴露 `IAudioProcessor`；`wvst-host-worker component-probe` 可继续作为隔离探测命令使用。
- `wvst-vst3-host` 已加入纯 Rust `Vst3Lifecycle` 状态机和 `Vst3ProcessingConfig`，覆盖 `created -> initialized -> setup-done -> activated -> processing -> stopped -> terminated` 的合法顺序和非法转移测试。
- `wvst-vst3-host` 已加入 `Vst3ProcessBuffers`，能预分配 planar input/output buffer，将 interleaved f32 输入转换为 VST3 channel buffers，并把 planar 输出复制回 interleaved f32；同时覆盖无输入音源 VST 的 buffer 路径。
- `wvst-vst3-host` 已加入 VST3 `IEventList` / `Event` ABI skeleton 和 `Vst3EventList` safe wrapper，`ProcessData.input_events` 现在指向稳定的预分配事件列表，每个 block 会清空旧事件并填充新的 note/poly pressure 事件。
- `wvst-vst3-host` 已加入 `Vst3AudioProcessor` facade，能封装 owned `IAudioProcessor` 指针并调用 `canProcessSampleSize`、`setupProcessing`、`setProcessing`、`process`、latency/tail 查询；fake ABI fixture 已覆盖真实 `ProcessData` 指针链路。
- `wvst-vst3-host` 已加入 `Vst3ComponentInstance` holder，能持有 owned `IComponent` + `Vst3AudioProcessor`，并将 initialize、setupProcessing、setActive、setProcessing、process、terminate 串入 `Vst3Lifecycle`；fake component/processor fixture 已覆盖完整生命周期和错误传播。
- `wvst-vst3-host` 已接入基础 audio bus 配置：`setupProcessing` 前调用 `setBusArrangements` 设置 mono/stereo 或 zero-input instrument arrangement，`activate/terminate` 会开关主 audio input/output bus，并覆盖 `setActive` 失败后的 bus rollback。
- macOS factory runtime 已提供 `create_vst3_component_instance()`，可通过 `IPluginFactory::createInstance(IComponent)` 和 `queryInterface(IAudioProcessor)` 创建 `Vst3LoadedComponent`，并保持 bundle 生命周期覆盖 component/processor holder。
- `wvst-host-worker serve` 已接入首版 runtime backend：instance create 可持久保存 `Vst3LoadedComponent`，`instance.start/stop/destroy` 会驱动真实 VST3 lifecycle，worker audio IPC 可调用真实 `process()`；invalid class id 或非 bundle 路径仍回退 passthrough 以保持测试和开发路径可用。
- worker create response、worker metrics、Bridge instance record 和 Web SDK `InstanceDescriptor` 已暴露 backend、`latencySamples`、`tailSamples`，Web 侧可以在挂载后读取插件处理延迟和 tail 信息。

## 距离完整能力的主要差距

### 1. 插件实例运行态生命周期

仍缺少：

- `ready` 之后的 `processing` / `stopped` 生命周期已有首版控制 API；仍缺少 `starting`、`stopping`、自动恢复中等瞬态状态和事件推送。
- 每个实例的独立 worker 进程已具备原型，并支持手动 restart；仍缺少自动 restart 状态机、崩溃事件推送和策略化资源回收。
- 同一插件 N 个实例的 worker 池化、调度和资源上限策略；当前更接近一实例一 worker 的保守隔离原型。

### 2. 持久 worker IPC

当前 Bridge 已能为实例启动并绑定一个常驻 worker 进程，控制面仍是 JSON-line 原型，音频面已有独立二进制 IPC。

仍缺少：

- 更完整的 Bridge worker supervisor 生命周期管理，包括自动 restart policy、主动 crash event 推送和 quarantine 解除策略。
- 正式 framed control IPC，替换当前 JSON-line 控制面原型。
- 超时后的全链路 kill/wait 审计、restart 指标和 crash quarantine 解除策略。
- 更完整的 worker capability negotiation，包括按数据面、MIDI、参数自动化和诊断能力分层协商。

### 3. 真实 VST3 component/controller lifecycle

当前已完成 factory info 读取、`IPluginFactory::createInstance` component probe、`IAudioProcessor` 接口探测、基础 ABI skeleton、纯 Rust lifecycle 状态机、processor facade、component holder、macOS factory 到 holder 的创建路径，以及 worker runtime backend 接入。

仍缺少：

- 完整 host context、多 bus arrangement、bus 查询和 active bus 策略；当前 holder 仍使用 null host context，且仅支持主 mono/stereo audio bus 与 zero-input instrument。
- controller 对象仍未接入，参数、state、program list、unit metadata 仍缺少。
- 真实第三方插件兼容验证仍不足；当前 `setProcessing`、`process`、latency/tail 主要由 fake ABI fixture、worker passthrough 和 Bridge runtime-info 传播测试覆盖。

### 4. 低延迟音频数据面

当前 Bridge 二进制帧已具备按 `streamId` 到 worker 二进制 audio IPC 的 passthrough 原型路由，但仍不是最终低延迟数据面。

仍缺少：

- stream open/close 已有首版控制 API；仍缺少 end-of-stream 帧语义、close 后 drain 策略和 WebAudio 端自动重开策略。
- Web Worker 从 SAB 取音频块并编码发送已有基础 ring-buffer audio pump；仍缺少更完整的延迟配置、调度调优和丢帧策略。
- Web 设备选择已有底层 helper、高层 session graph helper、capability API、device watcher、sample-rate guard、手动 stream restart 和基础 loopback metrics；仍缺少 sample-rate change 后的自动重建策略和真实端到端设备切换测量。
- Bridge 到 worker 的二进制 audio IPC 已具备首版；Bridge/Web 二进制诊断帧已有基础 flags，仍缺少共享内存/预分配 buffer 和背压语义。
- worker 路径已验证 sample rate / max block / processing state，并预分配输入/输出 sample scratch buffers、复用请求/响应 body buffer；runtime backend 已接入真实 VST `process()`，但当前仍经 worker instance mutex 串行处理，并保留 interleaved/planar scratch copy。
- late/drop/underflow/overflow 策略和 p50/p95/p99 指标；当前只有 route/fallback/failure 计数，还没有时延分位数。

### 5. MIDI 与音源 VST

仍缺少：

- MIDI/note event schema、Web/Bridge/worker 数据面传输和 VST3 `IEventList` note/poly pressure 转换已有首版；仍缺少 CC、pitch bend、channel aftertouch 到 VST3 parameter/controller path 的正式映射。
- 更完整的按 block sequence + sample offset 事件排序、背压和 late-event 策略。
- 音源 VST 的 zero-input audio buffer/session/passthrough worker plumbing 已有首版；note on/off 已能随 block 进入真实 VST3 `process()`，仍缺少真实第三方 instrument 兼容测试和 timing 诊断。
- Web MIDI adapter 和虚拟键盘示例。

### 6. 嵌入式 runtime 与打包

仍缺少：

- `wvst-embed` 或等价可嵌入 API。
- macOS 安装、启动、授权、日志和诊断命令。
- Windows/Linux worker supervision backend。

## 建议下一阶段

1. 补齐 host context、bus 查询和多声道 `setBusArrangements` 策略，并用真实 macOS VST3 effect fixture 验证 2-in/2-out `process()`。
2. 给 worker runtime backend 增加真实插件测试 fixture、兼容失败诊断和更细粒度 runtime capability。
3. 给 Bridge worker supervisor 增加自动 restart policy、quarantine 解除策略和 worker crash 事件回传。
4. 把 worker JSON-line 控制 IPC 抽象为可替换 framed control IPC，并扩展 capability negotiation。
5. 为 audio IPC 增加 backpressure/late-frame 指标和 p50/p95/p99 延迟统计。
