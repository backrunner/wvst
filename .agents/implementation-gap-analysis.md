# WVST Implementation Gap Analysis

更新日期：2026-06-03

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
- Bridge `instance.status` 已加入可配置的 worker 自动恢复策略：heartbeat 失败后默认尝试重启同一实例并保留 `instanceId` / `streamId`，如果实例原先处于 `processing` 会重新进入 processing；Bridge metrics 已暴露 `workerAutoRestarts`。
- Bridge/Web SDK 已提供 `instance.start` / `instance.stop` 处理生命周期控制，实例状态可从 `ready` 切到 `processing` / `stopped`，并已补入 `starting` / `stopping` / `recovering` 瞬态状态。
- Bridge 已提供运行时事件总线和 `bridge.events` 控制面查询；Web SDK 已暴露 `client.events()`，可观察 server lifecycle、worker start/ready/processing/stopped/failed/recovering/recovered/quarantine 事件。
- Bridge worker supervisor 已提供 quarantine TTL 释放策略，过期释放会清空累计失败计数并可通过事件观测。
- Bridge 音频路由现在要求实例处于 `processing` 状态；未 start、已 stop 或处理失败都会返回带 `silence` / `process-error` 的诊断静音帧，而不是继续把音频送进 worker。
- Instance heartbeat 已避免把正在 `processing` 的实例误降回 `ready`，降低控制面状态刷新对数据面的干扰。
- Bridge worker supervisor 已校验 `worker.hello` 中的 `ipcVersion`、`instanceLifecycle` 和 `binaryAudioProcess` capability，避免 Bridge 与不兼容 worker 继续创建实例。
- Bridge 创建 worker 实例时已把 `sampleRate` 和 `maxBlockFrames` 传给 worker；worker audio IPC 会验证 sample rate、最大 block、输入通道数和 processing 状态。
- `wvst-host-worker` 已为每个实例预分配 audio scratch buffers，并复用 audio IPC request/response body buffers；passthrough audio IPC 不再为每个 block 重复分配输入/输出 sample Vec 或响应 frame Vec。
- Bridge 二进制音频帧已能按 `streamId` 路由到对应 worker 的独立二进制 audio IPC，并回传 worker 处理后的 F32 frame；未匹配实例或非法帧暂时保留 echo fallback。
- Bridge metrics 已区分二进制帧总量、成功路由音频帧、fallback echo 和音频路由失败，便于后续接入 drop/late/underflow/overflow 统计。
- Bridge metrics 已加入二进制音频路由耗时直方图和 `audioRouteLatency` p50/p95/p99 微秒级快照，Web SDK metrics 类型已同步。
- Bridge 已加入按 stream 的音频序号诊断 tracker，能统计 sequence gap 事件/缺失帧估算、重复帧、乱序帧、late flag 和 interarrival jitter 分位数；stream close/open/destroy 会重置 tracker，避免 Web 端重开流后的误报。
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
- `wvst-protocol` 和 Web SDK 已新增固定 16 字节 parameter automation event schema，audio frame header 使用保留字段携带 `parameterEventCount`，payload 现在可表达 audio samples + MIDI events + parameter events。
- Rust audio frame 协议已允许 `channels = 0`，passthrough worker 路径已支持 zero-input instrument frame 并覆盖测试。
- `wvst-vst3-host` 已增加 VST3 FUID 规范化、`IPluginFactory::createInstance` ABI skeleton 和 macOS `create_vst3_component_probe()` safe facade；`wvst-host-worker component-probe <plugin.vst3> <class-id>` 可在隔离 worker 内验证 component 创建并释放。
- VST3 ABI 边界已补入 `IPluginBase`、`IComponent`、`IAudioProcessor`、`IHostApplication`、`ProcessSetup`、`AudioBusBuffers` 和 `ProcessData` 的 Rust repr(C) skeleton，`queryInterface` 已改为传递 16-byte TUID 而非 FUID 字符串，后续真实 process path 可以继续在 `wvst-vst3-host` 内收敛 unsafe。
- `create_vst3_component_probe()` 现在会通过 `queryInterface` 验证 component 是否暴露 `IAudioProcessor`；`wvst-host-worker component-probe` 可继续作为隔离探测命令使用。
- `wvst-vst3-host` 已加入纯 Rust `Vst3Lifecycle` 状态机和 `Vst3ProcessingConfig`，覆盖 `created -> initialized -> setup-done -> activated -> processing -> stopped -> terminated` 的合法顺序和非法转移测试。
- `wvst-vst3-host` 已加入 `Vst3ProcessBuffers`，能预分配 planar input/output buffer，将 interleaved f32 输入转换为 VST3 channel buffers，并把 planar 输出复制回 interleaved f32；同时覆盖无输入音源 VST 的 buffer 路径。
- `wvst-vst3-host` 已加入 VST3 `IEventList` / `Event` ABI skeleton 和 `Vst3EventList` safe wrapper，`ProcessData.input_events` / `output_events` 现在都指向稳定的预分配事件列表，每个 block 会清空旧事件；输入侧会填充新的 note/poly pressure 事件，输出侧可捕获插件通过 `addEvent` 写回的事件。
- `wvst-vst3-host` 已加入 VST3 `IParameterChanges` / `IParamValueQueue` ABI skeleton 和 `Vst3ParameterChanges` safe wrapper，`ProcessData.input_parameter_changes` / `output_parameter_changes` 现在都指向稳定的 host-owned 参数队列；输入侧会按 ParamID 分组填充 sample-accurate normalized automation points，输出侧可捕获插件通过 `addParameterData/addPoint` 写回的参数变化。
- `wvst-vst3-host` 已加入 VST3 host-owned `ProcessContext` 和 `IProcessContextRequirements` 查询，`ProcessData.process_context` 指向稳定堆内存并随 block 推进 sample timeline、tempo、拍号和 musical position；worker diagnostics 已暴露插件声明的 process context requirements bitmask。
- `wvst-vst3-host` 已加入 `Vst3AudioProcessor` facade，能封装 owned `IAudioProcessor` 指针并调用 `canProcessSampleSize`、`setupProcessing`、`setProcessing`、`process`、latency/tail 查询；fake ABI fixture 已覆盖真实 `ProcessData` 指针链路。
- `wvst-vst3-host` 已加入 `Vst3ComponentInstance` holder，能持有 owned `IComponent` + `Vst3AudioProcessor`，并将 initialize、setupProcessing、setActive、setProcessing、process、terminate 串入 `Vst3Lifecycle`；fake component/processor fixture 已覆盖完整生命周期和错误传播。
- `wvst-vst3-host` 已接入基础 audio bus 配置：`setupProcessing` 前调用 `setBusArrangements` 设置 mono/stereo 或 zero-input instrument arrangement，`activate/terminate` 会开关主 audio input/output bus，并覆盖 `setActive` 失败后的 bus rollback。
- `wvst-vst3-host` 已补入 VST3 `BusInfo` ABI 和 `Vst3AudioBusInfo` safe facade，component holder 可查询 audio input/output bus count、channel count、bus type、default active flag 和 UTF-16 bus name。
- component holder 的 audio bus activation 已从固定 index 0 改为基于查询结果选择 bus：优先匹配目标 channel count 的 main bus，其次 default-active main bus，再回退到第一个可用 bus。
- macOS factory runtime 已提供 `create_vst3_component_instance()`，可通过 `IPluginFactory::createInstance(IComponent)` 和 `queryInterface(IAudioProcessor)` 创建 `Vst3LoadedComponent`，并保持 bundle 生命周期覆盖 component/processor holder。
- `Vst3ComponentInstance::initialize()` 已传入 WVST `IHostApplication` host context，插件可通过 `queryInterface(IHostApplication)` 读取宿主名称；host-side `createInstance()` 已支持创建 host-owned `IMessage` 和 `IAttributeList` 对象，覆盖 controller/editor communication 常见宿主对象请求。
- `wvst-host-worker serve` 已接入首版 runtime backend：instance create 可持久保存 `Vst3LoadedComponent`，`instance.start/stop/destroy` 会驱动真实 VST3 lifecycle，worker audio IPC 可调用真实 `process()`；invalid class id 或非 bundle 路径仍回退 passthrough 以保持测试和开发路径可用。
- worker create response、worker metrics、Bridge instance record 和 Web SDK `InstanceDescriptor` 已暴露 backend、`latencySamples`、`tailSamples`，Web 侧可以在挂载后读取插件处理延迟和 tail 信息。
- VST3 controller 基础链路已接入：component 可查询 controller class id，macOS factory runtime 会创建可选 `IEditController`，worker 初始化 controller、注册可记录 begin/perform/end edit、restartComponent、dirty/editor/group-edit 的 `IComponentHandler`/`IComponentHandler2`，并在 component/controller 都支持 `IConnectionPoint` 时建立/释放双向连接；Bridge/worker/Web 控制面已暴露参数列表、unit/program metadata、normalized 参数读写、component/controller state base64 get/set 聚合、unit selection、unit-by-bus 查询、`setUnitProgramData`、`IProgramListData` 和 `IUnitData` 数据读写。
- `wvst-vst3-host` 已提供 `IBStream` 内存流、`IComponentHandler`/`IComponentHandler2` host callback 事件快照、`IConnectionPoint` component/controller 通信 facade 和 `Vst3EditController` safe facade，并用 fake ABI 覆盖参数信息、参数设置、state 写入、handler edit/restart/dirty/editor/group-edit callbacks、连接点 connect/disconnect 和生命周期释放。
- `wvst-host-worker` metrics 已暴露 VST3 runtime diagnostics，其中包含 controller `IComponentHandler` 最近事件和累计事件数；Web SDK metrics 类型已同步，后续 Web 主动推送/参数同步可以复用该结构。
- `wvst-vst3-host` 已提供可选 `IUnitInfo` facade，能读取 units、program lists、program names 和 selected unit；`wvst-host-worker` / Bridge / Web SDK 已提供 `instance.units` / `client.instances.units()` 查询 API。
- `wvst-vst3-host` 已提供可选 `IMidiMapping` facade；`wvst-host-worker` 会在 VST3 runtime 初始化后缓存 channel/controller 到 ParamID 的映射，并将 MIDI CC、pitch bend 和 channel aftertouch 转换为 VST3 parameter changes 随当前 audio block 输入。
- VST3 runtime process path 已将插件写回的 output note on/off、poly pressure 和 output parameter changes 规范化为 WVST 协议事件，并由 worker audio IPC 在响应 frame 中编码为 audio + MIDI event section + parameter automation section；未知或越界 VST3 output event 会被过滤，避免污染 Web 数据面。
- Workspace 已新增 `wvst-embed` crate，提供可嵌入 `BridgeRuntime` / `BridgeHandle`，支持应用内启动 Bridge Server、读取绑定地址、主动 shutdown、runtime event subscription、最近事件快照、外部 worker executable 注入和 worker timeout 配置。

## 距离完整能力的主要差距

### 1. 插件实例运行态生命周期

仍缺少：

- `ready` 之后的 `processing` / `stopped` 生命周期已有控制 API，`starting`、`stopping`、自动恢复中等瞬态状态和事件推送已有首版；仍缺少客户端侧长连接主动事件推送协议和更细粒度生命周期事件。
- 每个实例的独立 worker 进程已具备原型，并支持手动 restart 与 heartbeat 驱动的自动 restart；崩溃/恢复/quarantine 事件已有首版，仍缺少更完整的策略化资源回收和应用级资源上限。
- 同一插件 N 个实例的 worker 池化、调度和资源上限策略；当前更接近一实例一 worker 的保守隔离原型。

### 2. 持久 worker IPC

当前 Bridge 已能为实例启动并绑定一个常驻 worker 进程，控制面仍是 JSON-line 原型，音频面已有独立二进制 IPC。

仍缺少：

- 更完整的 Bridge worker supervisor 生命周期管理已有恢复中状态、事件快照和 quarantine 解除策略首版；仍缺少真正的 WebSocket server-push 事件订阅、进程树 kill/wait 审计和更多失败分类。
- 正式 framed control IPC，替换当前 JSON-line 控制面原型。
- 超时后的全链路 kill/wait 审计、细粒度 restart 诊断和 crash quarantine 策略调优。
- 更完整的 worker capability negotiation，包括按数据面、MIDI、参数自动化和诊断能力分层协商。

### 3. 真实 VST3 component/controller lifecycle

当前已完成 factory info 读取、`IPluginFactory::createInstance` component probe、`IAudioProcessor` 接口探测、基础 ABI skeleton、纯 Rust lifecycle 状态机、processor facade、component holder、macOS factory 到 holder 的创建路径，以及 worker runtime backend 接入。

仍缺少：

- 更完整的多 bus arrangement 和 process buffer 映射；当前 holder 已提供基础 `IHostApplication`、host-created `IMessage` / `IAttributeList`、audio bus 查询、selected-bus activation，并支持单个主 bus 的 mono/stereo/常见 3.0 到 7.1 speaker arrangement。
- `IEditController`、`IComponentHandler`/`IComponentHandler2` callback 事件记录、`IConnectionPoint`、参数列表、unit/program metadata、normalized 参数读写、component/controller state get/set 聚合、unit selection、unit-by-bus、program/unit data 读写以及 parameter-change queue/sample-accurate automation 已有首版；仍缺少真实第三方 controller/automation/unit-info/program-data/message/connection-point 兼容验证，以及把 handler 事件主动推送到 Web UI 的协议。
- 真实第三方插件兼容验证仍不足；当前 `setProcessing`、`process`、process context、latency/tail 主要由 fake ABI fixture、worker passthrough 和 Bridge runtime-info 传播测试覆盖。

### 4. 低延迟音频数据面

当前 Bridge 二进制帧已具备按 `streamId` 到 worker 二进制 audio IPC 的 passthrough 原型路由，但仍不是最终低延迟数据面。

仍缺少：

- stream open/close 已有首版控制 API；仍缺少 end-of-stream 帧语义、close 后 drain 策略和 WebAudio 端自动重开策略。
- Web Worker 从 SAB 取音频块并编码发送已有基础 ring-buffer audio pump；仍缺少更完整的延迟配置、调度调优和丢帧策略。
- Web 设备选择已有底层 helper、高层 session graph helper、capability API、device watcher、sample-rate guard、手动 stream restart 和基础 loopback metrics；仍缺少 sample-rate change 后的自动重建策略和真实端到端设备切换测量。
- Bridge 到 worker 的二进制 audio IPC 已具备首版；Bridge/Web 二进制诊断帧已有基础 flags，仍缺少共享内存/预分配 buffer 和背压语义。
- worker 路径已验证 sample rate / max block / processing state，并预分配输入/输出 sample scratch buffers、复用请求/响应 body buffer；runtime backend 已接入真实 VST `process()`，且 VST3 host 已捕获插件写回的 output events/parameter changes 并编码回 Web 响应帧；当前仍经 worker instance mutex 串行处理、保留 interleaved/planar scratch copy，且未知 VST3 output event type 仍只做过滤不做 Web 侧扩展表达。
- late/drop/jitter 首版 Bridge 诊断指标已完成；仍缺少 WebAudio 端 underflow/overflow 与 Bridge 序号指标的统一策略、端到端 WebAudio 往返延迟测量和共享内存/背压语义。

### 5. MIDI 与音源 VST

仍缺少：

- MIDI/note event schema、Web/Bridge/worker 数据面传输、VST3 `IEventList` note/poly pressure 转换，以及 CC、pitch bend、channel aftertouch 经 `IMidiMapping` 到 VST3 parameter-change path 的映射已有首版。
- 更完整的按 block sequence + sample offset 事件排序、背压和 late-event 策略。
- 音源 VST 的 zero-input audio buffer/session/passthrough worker plumbing 已有首版；note on/off 已能随 block 进入真实 VST3 `process()`，仍缺少真实第三方 instrument 兼容测试和 timing 诊断。
- Web MIDI adapter 和虚拟键盘示例。

### 6. 嵌入式 runtime 与打包

仍缺少：

- `wvst-embed` 已有 runtime builder、事件订阅、事件快照、外部 worker executable 注入和 timeout 配置；仍缺少应用生命周期集成示例、日志/诊断集成和打包脚本。
- macOS 安装、启动、授权、日志和诊断命令。
- Windows/Linux worker supervision backend。

## 建议下一阶段

1. 用真实 macOS VST3 effect/instrument fixture 验证 2-in/2-out process、parameter automation、MIDI mapping 和 zero-input instrument timing。
2. 用真实第三方插件验证 controller/automation/unit-info/program-data/message/attribute 兼容性。
3. 给 worker runtime backend 增加兼容失败诊断和更细粒度 runtime capability。
4. 把 worker JSON-line 控制 IPC 抽象为可替换 framed control IPC，并扩展 capability negotiation。
5. 增加 WebSocket server-push 事件订阅、进程树 kill/wait 审计和资源上限策略。
6. 将 Bridge audio sequence/late/jitter 指标与 WebAudio worker/worklet underflow/overflow 指标打通，并把 Bridge route latency 扩展到端到端 WebAudio 往返测量。
