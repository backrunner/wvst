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
- `wvst-host-worker serve-framed` 已提供常驻 framed control IPC，使用 `WVCI` 固定 header + JSON body 承载 worker hello、instance lifecycle 和参数/状态控制请求；`serve` JSON-line 模式仍保留给测试 fixture 和开发调试。音频处理已从 debug JSON 小块处理迁移到独立二进制 audio IPC。
- Bridge `instance.create` 已能默认启动并绑定 `wvst-host-worker serve-framed`，通过 `framedControlIpc` capability 校验后进入 `ready` / `ready` 状态。
- Bridge worker supervisor 已加入 stderr 摘要、启动失败计数和基础 quarantine，避免同一故障插件无限重启。
- Bridge worker supervisor 已加入应用级 worker 实例数量上限，默认最多 64 个活跃 worker，可通过 `WVST_MAX_WORKER_INSTANCES` 或 runtime builder 配置；超限时控制面返回结构化 `resource-limit-exceeded` 错误并清理未启动实例记录。
- Bridge/Web SDK 已提供 `instance.status` heartbeat API，能通过 worker `worker.metrics` 检查实例 worker 存活，并在 worker 退出或 IPC 断开时把实例标记为 `failed`。
- Bridge/Web SDK 已提供 `instance.restart` 手动恢复 API，能在保留 `instanceId` / `streamId` 的情况下杀掉旧 worker 并重新拉起同一实例；Bridge metrics 已暴露 `workerFailures` 和 `workerRestarts`。
- Bridge `instance.status` 已加入可配置的 worker 自动恢复策略：heartbeat 失败后默认尝试重启同一实例并保留 `instanceId` / `streamId`，如果实例原先处于 `processing` 会重新进入 processing；Bridge metrics 已暴露 `workerAutoRestarts`。
- Bridge worker supervisor 已接入 kill/wait shutdown audit metrics，`bridge.metrics` / Web SDK 可观测 `workerShutdowns`、`workerKillRequests`、`workerTreeKillRequests`、`workerForcedKillRequests`、`workerWaitSuccesses` 和 `workerWaitTimeouts`。
- Bridge/Web SDK 已提供 `instance.start` / `instance.stop` 处理生命周期控制，实例状态可从 `ready` 切到 `processing` / `stopped`，并已补入 `starting` / `stopping` / `recovering` 瞬态状态。
- Bridge 已提供运行时事件总线、`bridge.events` 控制面查询和授权后 WebSocket `bridge.event` server-push notification；Web SDK 已暴露 `client.events()` 轮询和 `client.onEvent()` 主动订阅，可观察 server lifecycle、worker start/ready/processing/stopped/failed/recovering/recovered/quarantine 事件；`worker-failed` 事件会携带可选 `errorData`，数据面 audio process 失败也会发布包含 worker/runtime 结构化原因的失败事件。
- Bridge worker supervisor 已提供 quarantine TTL 释放策略，过期释放会清空累计失败计数并可通过事件观测；quarantine error data 与 `worker-quarantined` event 已暴露 `releaseAfterMs`，便于 Web/UI 展示重试倒计时和策略诊断；`worker-recovering` / `worker-recovered` / `worker-recovery-failed` event 已通过 `mode`、`reason` 和结构化 `errorData` 区分手动 restart、heartbeat 自动恢复、重启失败与 processing 恢复失败。
- Bridge worker supervisor 的 quarantine failure threshold 已可通过 `WVST_WORKER_QUARANTINE_FAILURES`、`BridgeConfig::with_worker_quarantine_failure_threshold()` 和 `BridgeRuntimeBuilder::worker_quarantine_failure_threshold()` 配置；嵌入式宿主可按自身恢复策略调节插件进入 quarantine 前允许的失败次数。
- Bridge worker supervisor 已提供首版 worker 进程树终止：Unix/macOS 启动 worker 时放入独立 process group，shutdown 时优先向 process group 发终止信号并等待，超时后升级强制 kill；测试覆盖 worker 派生子进程后 destroy 仍能清理进程组。
- Bridge worker supervisor 生产路径已默认通过 `WVCI` framed control IPC 发送/接收 worker 控制请求；worker hello 会暴露并校验 `framedControlIpcVersion`、`framedControlMaxBodyBytes`、sequence id、status code 和 error response frame capability，并暴露 `framedControlBatching`。测试覆盖真实 worker framed create/destroy passthrough path、版本不匹配和 body cap/capability 不匹配拒绝路径。framed control IPC 已能在 header 层区分 `Response` / `ErrorResponse`，worker JSON-RPC error 会映射到非零 `statusCode`，Bridge 会校验 frame kind、sequence 和 status 后再解析 body；协议、worker serve-framed 与 Bridge supervisor 已支持 `BatchRequest` / `BatchResponse`，可在一个外层 frame 内承载多个独立 sequence 的控制请求/响应。
- Bridge 音频路由现在要求实例处于 `processing` 状态；未 start、已 stop 或处理失败都会返回带 `silence` / `process-error` 的诊断静音帧，而不是继续把音频送进 worker。
- Instance heartbeat 已避免把正在 `processing` 的实例误降回 `ready`，降低控制面状态刷新对数据面的干扰。
- Bridge worker supervisor 已校验 `worker.hello` 中的 `ipcVersion`、`instanceLifecycle` 和 `binaryAudioProcess` capability，避免 Bridge 与不兼容 worker 继续创建实例。
- Bridge 创建 worker 实例时已把 `sampleRate` 和 `maxBlockFrames` 传给 worker；worker audio IPC 会验证 sample rate、最大 block、输入通道数和 processing 状态。
- `wvst-host-worker` 已为每个实例预分配 audio scratch buffers，并复用 audio IPC request/response body buffers；passthrough audio IPC 不再为每个 block 重复分配输入/输出 sample Vec 或响应 frame Vec。
- Bridge 二进制音频帧已能按 `streamId` 路由到对应 worker 的独立二进制 audio IPC，并回传 worker 处理后的 F32 frame；未匹配实例或非法帧暂时保留 echo fallback。
- Bridge metrics 已区分二进制帧总量、成功路由音频帧、fallback echo 和音频路由失败，便于后续接入 drop/late/underflow/overflow 统计。
- Bridge metrics 已加入二进制音频路由耗时直方图和 `audioRouteLatency` p50/p95/p99 微秒级快照，Web SDK metrics 类型已同步。
- Bridge 已加入按 stream 的音频序号诊断 tracker，能统计 sequence gap 事件/缺失帧估算、重复帧、乱序帧、late flag 和 interarrival jitter 分位数；stream close/open/destroy 会重置 tracker，避免 Web 端重开流后的误报。
- Bridge 音频路由已加入每 stream in-flight limiter：同一 stream 的上一块音频仍在 worker 处理时，新 block 会被判定为 backpressure drop，并返回带 `silence` / `late` flag 的诊断静音帧；Bridge/Web metrics 已暴露 `audioBackpressureDrops`。
- Bridge/Web SDK 已提供 `stream.open` / `stream.close` 控制 API，实例记录包含 `streamState`，Bridge 只将 open stream 的音频帧路由到 worker。
- 对于已知 stream 的关闭或处理失败场景，Bridge 会返回带 `silence` / `end-of-stream` / `process-error` flag 的诊断静音音频帧，避免把异常伪装成正常 echo；`stream.close` 控制面会等待同 stream 的在途音频块释放或超时后再返回，避免 close 与最后一个 block 竞争；Bridge event bus 已发布 `stream-opened`、`stream-closing` 和带 `drainTimedOut` 的 `stream-closed` 事件。
- Web `bridge-worker` 已具备从 SAB input ring 读取 quantum、编码 WVST binary audio frame、发送 Bridge 并写回 output ring 的基础 audio pump；AudioWorklet processor 已支持通过 SAB ring 和计数器交换音频块。
- Web SDK 已提供 `WVSTBridgeWorkerClient`，封装 bridge worker 的 connect/request/sendBinary/startAudioStream/stopAudioStream 命令，避免应用侧手写 worker message protocol。
- Web SDK 已提供音频设备选择 helper：可枚举 `audioinput`/`audiooutput`，按 `deviceId` 请求输入 `MediaStream`，创建 `MediaStreamAudioSourceNode`，并通过 `AudioContext.setSinkId()` 或 `MediaStreamAudioDestinationNode + HTMLMediaElement.setSinkId()` 指定输出设备。
- Web SDK 已提供 `createWVSTAudioDeviceSession()` 高层 session graph helper，可按 Web 指定的输入/输出设备把 `MediaStreamAudioSourceNode`、WVST AudioWorklet/SAB、Bridge worker audio pump 和 media-element output route 串起来，并支持 0-input 音源 VST 的 Web 数据面启动。
- Web SDK 已提供音频设备 capability API、`devicechange` watcher，以及 session 运行期 `setInputDevice()` / `setOutputDevice()` 切换方法。
- Web session 已校验 `AudioContext.sampleRate` 与 instance `sampleRate` 一致，提供 `restartAudioStream()` 重启 Bridge worker audio pump，并通过 `getMetrics()` 暴露 loopback underflow/overflow 和 pending quantum 指标。
- `wvst-protocol` 和 Web SDK 已定义固定 16 字节 MIDI/note event schema，事件包含 sample offset、kind、channel、data bytes 和 note id；audio frame payload 已能表达 audio samples 后追加 event section。
- Web SDK 已提供 `sendMidiEvents()` 路径，DedicatedWorker 会缓存 MIDI events、按 sample offset 排序，并随下一块 audio frame 发送到 Bridge；Bridge/worker 音频 IPC 已能接受 event section，worker 会将 note on/off、poly pressure 和对应 raw MIDI note 事件转换为 VST3 `IEventList` 输入。
- `wvst-protocol` 和 Web SDK 已新增固定 16 字节 parameter automation event schema，audio frame header 使用保留字段携带 `parameterEventCount`，payload 现在可表达 audio samples + MIDI events + parameter events。
- worker 音频 IPC 边界已对输入 MIDI/note events、parameter automation、MIDI mapping 产生的参数变化，以及插件输出回 Web 的 MIDI/parameter events 按 block 内 sample offset 做确定性排序；同 sample offset 的 MIDI/note events 保留原始相对顺序，parameter changes 再按 ParamID 稳定归并。
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
- `wvst-host-worker runtime-probe <plugin.vst3> <class-id>` 已提供隔离进程内真实 VST3 runtime smoke 入口，可按指定 sample rate、block size、输入/输出通道、frames、blocks、MIDI note 和参数自动化输入创建 component/controller、完成 setup/activate/start/process/stop/terminate，并输出参数数量、audio bus、latency/tail、process context requirements、process timing 和 output event/parameter diagnostics JSON；process 失败时会尽量执行 stop/terminate 清理，便于第三方 effect/instrument 兼容验证自动化。
- `wvst-vst3-host` 已接入基础 audio bus 配置：`setupProcessing` 前调用 `setBusArrangements` 设置 mono/stereo 或 zero-input instrument arrangement，`activate/terminate` 会开关主 audio input/output bus，并覆盖 `setActive` 失败后的 bus rollback。
- `wvst-vst3-host` 已补入 VST3 `BusInfo` ABI 和 `Vst3AudioBusInfo` safe facade，component holder 可查询 audio input/output bus count、channel count、bus type、default active flag 和 UTF-16 bus name。
- component holder 的 audio bus activation 已从固定 index 0 改为基于查询结果选择 bus：优先匹配目标 channel count 的 main bus，其次 default-active main bus，再回退到第一个可用 bus。
- macOS factory runtime 已提供 `create_vst3_component_instance()`，可通过 `IPluginFactory::createInstance(IComponent)` 和 `queryInterface(IAudioProcessor)` 创建 `Vst3LoadedComponent`，并保持 bundle 生命周期覆盖 component/processor holder。
- `Vst3ComponentInstance::initialize()` 已传入 WVST `IHostApplication` host context，插件可通过 `queryInterface(IHostApplication)` 读取宿主名称；host-side `createInstance()` 已支持创建 host-owned `IMessage` 和 `IAttributeList` 对象，覆盖 controller/editor communication 常见宿主对象请求。
- `wvst-host-worker serve` 已接入首版 runtime backend：instance create 可持久保存 `Vst3LoadedComponent`，`instance.start/stop/destroy` 会驱动真实 VST3 lifecycle，worker audio IPC 可调用真实 `process()`；invalid class id 或非 bundle 路径仍回退 passthrough 以保持测试和开发路径可用。
- worker create response、worker metrics、Bridge instance record 和 Web SDK `InstanceDescriptor` 已暴露 backend、`latencySamples`、`tailSamples`，Web 侧可以在挂载后读取插件处理延迟和 tail 信息。
- VST3 controller 基础链路已接入：component 可查询 controller class id，macOS factory runtime 会创建可选 `IEditController`，worker 初始化 controller、注册可记录 begin/perform/end edit、restartComponent、dirty/editor/group-edit 的 `IComponentHandler`/`IComponentHandler2`，并在 component/controller 都支持 `IConnectionPoint` 时建立/释放双向连接；Bridge/worker/Web 控制面已暴露参数列表、unit/program metadata、normalized 参数读写、normalized/plain/display string 转换、component/controller state base64 get/set 聚合、unit selection、unit-by-bus 查询、`setUnitProgramData`、`IProgramListData` 和 `IUnitData` 数据读写。
- `wvst-vst3-host` 已提供 `IBStream` 内存流、`IComponentHandler`/`IComponentHandler2` host callback 事件快照、`IConnectionPoint` component/controller 通信 facade、`IConnectionPoint::notify(IMessage*)`、host-owned `IMessage` typed attributes helper 和 `Vst3EditController` safe facade，并用 fake ABI 覆盖参数信息、参数设置、state 写入、handler edit/restart/dirty/editor/group-edit callbacks、连接点 connect/disconnect/notify 和生命周期释放。
- `wvst-host-worker` metrics 已暴露 VST3 runtime diagnostics，其中包含 controller `IComponentHandler` 最近事件和累计事件数；Bridge 已在 `instance.status` heartbeat 路径上把新增 handler event 增量转换为 `bridge.event` server-push 事件，并能报告 recent-event ring 溢出导致的 lost sequence；`restartComponent` raw flags 已解码为结构化 `restartFlags`（latency/io/parameter title/value/MIDI mapping 等变化），Bridge 会额外发布 `vst3-metadata-invalidated` 事件指示 Web 侧刷新 parameter values/info、latency、I/O、MIDI mapping 等 metadata，并附带 `refreshPolicy` 区分 metadata refresh、audio graph rebuild 和 component reload；Web SDK 事件和 runtime diagnostics 类型已同步，并提供 `refreshMetadataForInvalidation()` / `onMetadataInvalidated()` helper 自动拉取受影响的 status、parameters、units 和可选 parameter values。
- Bridge 控制面已提供 `instance.metadata.refresh` 批量刷新入口，可通过 framed control batch 在一次 worker 往返中获取参数列表、unit metadata、可选 state 和默认 worker metrics；`instance.runtime.snapshot` 可进一步把实例记录、batch metadata refresh 和可选 recent events 聚合成一次 UI 挂载快照；JSON-line 测试 worker 会自动 fallback 为顺序请求。
- `wvst-host-worker` 已为每个实例缓存并暴露带 `schemaVersion` 的 `runtimeCapabilities`，Bridge `InstanceRecord` / Web SDK `InstanceDescriptor` / worker runtime metrics 均可读取当前实例对 binary audio、component/controller state、parameters、parameter automation、unit/program data、MIDI mapping、output events、component handler events、connection points 和 process context 的支持情况；passthrough runtime diagnostics 已能区分 missing class id、non-bundle path 和 invalid class id fallback 原因；worker JSON-RPC error 已支持结构化 `data`，Bridge 会把 worker rejection data 保留到 `error.data.workerData`，VST3 runtime init 失败可暴露 component create/initialize、controller initialize、setup、activate 阶段和 host error kind；audio IPC process error 已支持结构化 JSON body，VST3 `process()` 失败可透传 `vst3-runtime-process`、`component.process` 和 host error kind。worker control/audio IPC 构造端已校验 request/response status 必须为 0、error status 必须非 0，并在读入 body 前应用协议级最大 body 长度，避免损坏 header 触发大内存分配。
- VST3 参数、state、unit/program data、connection notify 和 lifecycle 控制面失败已通过 worker JSON-RPC `error.data` 暴露 `vst3-runtime-control`、stage、host error kind 和 message；Bridge 继续通过 `workerData` 原样透传给 Web。VST3 host `IBStream` state/data 导出已加入 8MiB bounded writable stream，Web/worker 输入侧 state、program data、unit data 和 connection message binary/string/attribute count 也已加上显式限制，Bridge WebSocket text 控制消息默认按 framed control IPC 16MiB 上限提前拒绝。
- `wvst-testkit` 已新增纯 Rust latency harness、stability runner 和 runtime probe matrix runner，可记录 sequence gap、duplicate/out-of-order、drop、timeout、late/silence/process-error 计数，并输出 route latency、round-trip frames 与 round-trip microseconds 的 p50/p95/p99 快照；runner 默认按 30 分钟稳定性窗口推进 block/sequence/frame timeline，runtime matrix runner 可用固定 case 清单调用隔离 worker 的 `runtime-probe` 批量记录 effect/instrument、MIDI note 和参数自动化 smoke 结果，后续 Bridge/Web/真实插件稳定性测试可复用。
- `wvst-vst3-host` 已提供可选 `IUnitInfo` facade，能读取 units、program lists、program names 和 selected unit；`wvst-host-worker` / Bridge / Web SDK 已提供 `instance.units` / `client.instances.units()` 查询 API。
- Web/Bridge/worker/VST3 facade 已提供 UI 发起的参数 edit gesture API：`instance.parameter.beginEdit`、`instance.parameter.performEdit`、`instance.parameter.endEdit`；`performEdit` 会调用 controller `setParamNormalized`，三类 gesture 会写入 component-handler event snapshot，且 VST3 host facade 已校验 `begin -> perform* -> end` 配对顺序，避免重复 begin 或无 begin 的 perform/end 污染参数事件流。
- Web/Bridge/worker/VST3 facade 已提供 component/controller connection-point notify API：`instance.connection.notifyComponent`、`instance.connection.notifyController` / `client.instances.notifyComponent()`、`client.instances.notifyController()`，Web 可传入 `messageId` 和 int/float/string/binary typed attributes，由 worker 构造 host-owned `IMessage` 后调用对应 VST3 connection point。
- `wvst-vst3-host` 已提供可选 `IMidiMapping` facade；`wvst-host-worker` 会在 VST3 runtime 初始化后缓存 channel/controller 到 ParamID 的映射，并将 MIDI CC、pitch bend 和 channel aftertouch 转换为 VST3 parameter changes 随当前 audio block 输入。
- VST3 runtime process path 已将插件写回的 output note on/off、poly pressure 和 output parameter changes 规范化为 WVST 协议事件，并由 worker audio IPC 在响应 frame 中编码为 audio + MIDI event section + parameter automation section；未知、越界或 payload 非法的 VST3 output event 会被过滤以避免污染 Web 数据面，同时最近一次 process 的 output event / output parameter-change raw、normalized、filtered 计数已进入 worker runtime diagnostics，便于真实插件兼容测试定位丢失原因。
- Workspace 已新增 `wvst-embed` crate，提供可嵌入 `BridgeRuntime` / `BridgeHandle`，支持应用内启动 Bridge Server、读取绑定地址、主动 shutdown、runtime event subscription、最近事件快照、外部 worker executable 注入和 worker timeout 配置。
- `wvst-embed` 的 `BridgeHandle` 已提供只读 metrics snapshot 和 runtime diagnostics 聚合，嵌入式宿主可直接读取本地地址、指标和 recent events 做健康检查/日志集成；`BridgeRuntimeLogRecord` 已提供 event/diagnostics JSON-lines 写入 helper，`crates/wvst-embed/examples/embedded_bridge.rs` 提供可编译的应用生命周期和结构化日志管线集成示例，覆盖配置、事件订阅、启动、diagnostics 读取和优雅 shutdown。
- Workspace 已新增 `wvst-process-supervision` crate，将 worker 进程树终止的 Unix process group 与 Windows Job Object 平台 FFI 收敛到独立安全 API；`wvst-bridge-server` 继续保持 `unsafe_code = deny`。
- Bridge worker supervisor 已接入可选 worker address-space memory cap 和 CPU time hard cap：`WVST_WORKER_MEMORY_LIMIT_BYTES` / `BridgeConfig::with_worker_memory_limit_bytes()` 与 `WVST_WORKER_CPU_TIME_LIMIT_SECONDS` / `BridgeConfig::with_worker_cpu_time_limit_seconds()` 会通过 `wvst-process-supervision::WorkerResourceLimits` 传给 worker spawn；Unix/macOS/Linux 在 child `exec` 前设置 `RLIMIT_AS` / `RLIMIT_CPU`，Windows Job Object 会设置 job memory 和 job user-time limit。Linux 还新增 cgroup v2 backend，可通过 `WVST_WORKER_LINUX_CGROUP_PARENT`、`WVST_WORKER_LINUX_CGROUP_MEMORY_MAX_BYTES`、`WVST_WORKER_LINUX_CGROUP_CPU_QUOTA_MICROS` 和 `WVST_WORKER_LINUX_CGROUP_CPU_PERIOD_MICROS` 为每个 worker 创建独立 cgroup 并写入 `memory.max` / `cpu.max` / `cgroup.procs`；cgroup 配置失败会以 `supervision-setup-failed` 结构化错误返回，Bridge Server 自身仍不含 unsafe。
- `wvst-bridge-server diagnose` 已提供本地 JSON 诊断命令，可输出 bridge version、平台信息、当前 env-derived config、token 是否启用但不泄露 token 值、host worker 路径/存在性/timeout 和 WVST 环境变量状态，便于 macOS 安装、启动和嵌入式宿主集成排查。

## 距离完整能力的主要差距

### 1. 插件实例运行态生命周期

仍缺少：

- `ready` 之后的 `processing` / `stopped` 生命周期已有控制 API，`starting`、`stopping`、自动恢复中等瞬态状态和事件推送已有首版；WebSocket server-push notification 已能把 Bridge event 主动发给授权 Web 客户端，start/stop/destroy 已补充更细粒度的 `worker-processing-starting`、`worker-processing-stopping`、`worker-destroying` 和 `worker-destroyed` 事件，stream open/close 已补充 `stream-opened`、`stream-closing` 和 `stream-closed` 事件；应用级 `worker-policy-decision` 事件已有首版，覆盖 quarantine release/reject、worker 实例资源上限拒绝、手动 restart、heartbeat 自动 restart 和自动恢复禁用拒绝，仍缺少更完整的策略回收/调度决策与真实压测校准。
- 每个实例的独立 worker 进程已具备原型，并支持手动 restart 与 heartbeat 驱动的自动 restart；崩溃/恢复/quarantine 事件、worker 实例数量上限、memory hard cap、CPU time hard cap 和 Linux cgroup v2 CPU/memory quota 已有首版，仍缺少更完整的策略化资源回收、真实 Linux cgroup 部署验证和池化调度策略。
- 同一插件 N 个实例的 worker 池化、调度和资源上限策略；当前更接近一实例一 worker 的保守隔离原型。

### 2. 持久 worker IPC

当前 Bridge 已能为实例启动并绑定一个常驻 worker 进程，控制面和音频面都已有独立 framed IPC；JSON-line 控制模式保留为测试 fixture / 调试兼容路径。

仍缺少：

- 更完整的 Bridge worker supervisor 生命周期管理已有恢复中状态、事件快照、WebSocket server-push 事件订阅、quarantine 解除策略、worker kill/wait 审计指标、Unix/macOS 进程组终止、Windows Job Object 终止、Unix `RLIMIT_AS` / `RLIMIT_CPU` worker hard cap、Windows Job Object memory/user-time hard cap、Linux cgroup v2 CPU/memory quota 和 framed control IPC 首版；仍缺少 Windows 真实运行验证、Linux cgroup 真实部署验证和更多失败分类。
- framed control IPC 已替换生产路径 JSON-line 控制面，并已有 schema version、max body、sequence id、status code、error response frame capability 校验、header-level error classification、sequence/status 校验、body size cap、wire-level 批处理/多路复用 frame、`instance.metadata.refresh` 的 Bridge 批量 metadata refresh 入口，以及 `instance.runtime.snapshot` 的实例挂载聚合入口；仍缺少把更多编辑/状态写入类组合调用迁移到 batch 调度。
- 超时后的全链路 kill/wait 审计已有首版 counters，且 Unix/macOS 已覆盖进程树维度；quarantine 已暴露释放倒计时诊断、可配置失败阈值，restart/recovery 事件已暴露手动/自动模式、触发原因、恢复失败原因和结构化错误，仍缺少基于真实插件压测数据的默认策略校准。
- worker hello 已有基础 capability negotiation 和 framed control IPC schema/body/capability 校验，实例级 `runtimeCapabilities` 已能按数据面、MIDI、参数自动化和诊断能力暴露首版，且包含 schema version 与 passthrough fallback 原因；worker rejection 已能透传 VST3 runtime init、control 和 `process()` 阶段化失败 data，framed IPC 已能在 header 层标记 worker rejection，并已提供批处理/多路复用 frame 和 Bridge 层批量 metadata refresh 入口；仍缺少更完整的非 VST3 runtime/control/process 失败分类。

### 3. 真实 VST3 component/controller lifecycle

当前已完成 factory info 读取、`IPluginFactory::createInstance` component probe、`IAudioProcessor` 接口探测、基础 ABI skeleton、纯 Rust lifecycle 状态机、processor facade、component holder、macOS factory 到 holder 的创建路径，以及 worker runtime backend 接入。

仍缺少：

- 更完整的多 bus arrangement 和 process buffer 映射；当前 holder 已提供基础 `IHostApplication`、host-created `IMessage` / `IAttributeList`、audio bus 查询、selected-bus activation，并支持单个主 bus 的 mono/stereo/常见 3.0 到 7.1 speaker arrangement。
- `IEditController`、`IComponentHandler`/`IComponentHandler2` callback 事件记录与 Bridge server-push、`IConnectionPoint` connect/disconnect/notify、参数列表、unit/program metadata、normalized 参数读写、带顺序校验的 UI 参数 edit gesture、normalized/plain/display string 转换、component/controller state get/set 聚合、unit selection、unit-by-bus、program/unit data 读写以及 parameter-change queue/sample-accurate automation 已有首版；metadata invalidation 已能区分 metadata refresh、audio graph rebuild 和 component reload policy；仍缺少真实第三方 controller/automation/unit-info/program-data/message notify/connection-point 兼容验证，以及真实 host latency/bus 变化后的应用层重建测量。
- 真实第三方插件兼容验证仍不足；当前 `setProcessing`、`process`、process context、latency/tail 主要由 fake ABI fixture、worker passthrough、Bridge runtime-info 传播测试、可对真实 bundle 执行的多 block `runtime-probe` smoke 入口和 `wvst-testkit` runtime matrix runner 覆盖，仍需要在 CI/本地纳入固定第三方插件清单与长时稳定性记录。

### 4. 低延迟音频数据面

当前 Bridge 二进制帧已具备按 `streamId` 到 worker 二进制 audio IPC 的 passthrough 原型路由，但仍不是最终低延迟数据面。

仍缺少：

- stream open/close 已有首版控制 API、end-of-stream 诊断帧和 close 后 in-flight drain；仍缺少 WebAudio 端自动重开策略。
- Web Worker 从 SAB 取音频块并编码发送已有基础 ring-buffer audio pump；仍缺少更完整的延迟配置、调度调优和丢帧策略。
- Web 设备选择已有底层 helper、高层 session graph helper、capability API、device watcher、sample-rate guard、手动 stream restart 和基础 loopback metrics；仍缺少 sample-rate change 后的自动重建策略和真实端到端设备切换测量。
- Bridge 到 worker 的二进制 audio IPC 已具备首版；Bridge 侧 audio IPC 请求 frame buffer 已随 worker audio connection 复用，且路由热路径会直接借用 WebSocket binary payload 编码 worker request，避免每个 audio block 额外复制输入帧；Bridge/Web 二进制诊断帧已有基础 flags，每 stream backpressure drop 会返回 `silence` / `late` 诊断帧并计数；仍缺少共享内存/更完整预分配响应 buffer、Web worker 侧主动 drop 策略和端到端背压协调。
- worker 路径已验证 sample rate / max block / processing state，并预分配输入/输出 sample scratch buffers、复用请求/响应 body buffer；runtime backend 已接入真实 VST `process()`，且 VST3 host 已捕获插件写回的 output events/parameter changes 并编码回 Web 响应帧，最近一次 process 的 output 过滤统计也会进入 runtime diagnostics；当前仍经 worker instance mutex 串行处理、保留 interleaved/planar scratch copy，且未知 VST3 output event type 仍只做过滤不做 Web 侧扩展表达。
- late/drop/jitter/backpressure 首版 Bridge 诊断指标已完成，`wvst-testkit` 已提供可复用 latency snapshot collector 与 30 分钟 stability runner；仍缺少 WebAudio 端 underflow/overflow 与 Bridge 序号指标的统一策略、真实端到端 WebAudio 往返延迟测量和共享内存数据面。

### 5. MIDI 与音源 VST

仍缺少：

- MIDI/note event schema、Web/Bridge/worker 数据面传输、VST3 `IEventList` note/poly pressure 转换，以及 CC、pitch bend、channel aftertouch 经 `IMidiMapping` 到 VST3 parameter-change path 的映射已有首版。
- Rust worker 边界已具备 block 内 sample offset 确定性排序；仍缺少 Web worker 侧背压和 late-event 策略。
- 音源 VST 的 zero-input audio buffer/session/passthrough worker plumbing 已有首版；note on/off 已能随 block 进入真实 VST3 `process()`，仍缺少真实第三方 instrument 兼容测试和 timing 诊断。
- Web MIDI adapter 和虚拟键盘示例。

### 6. 嵌入式 runtime 与打包

仍缺少：

- `wvst-embed` 已有 runtime builder、事件订阅、事件快照、只读 metrics diagnostics、外部 worker executable 注入、timeout 配置、JSON-lines 日志 helper 和可编译应用生命周期/日志管线示例；仍缺少打包脚本。
- macOS 安装、启动、授权和日志已有基础诊断命令支持；仍缺少发布安装脚本和更完整的日志采集/轮转命令。
- Windows 真实运行验证、Linux cgroup 资源限制 backend 和发布打包脚本。

## 建议下一阶段

1. 用 `wvst-testkit` runtime matrix runner 接入真实 macOS VST3 effect/instrument fixture，验证 2-in/2-out process、parameter automation、MIDI mapping 和 zero-input instrument timing。
2. 用真实第三方插件验证 controller/automation/unit-info/program-data/message notify/typed attribute 兼容性。
3. 给 worker runtime backend 增加兼容失败诊断，并继续细化 `runtimeCapabilities` 的失败原因和 schema versioning。
4. 将 framed control IPC 的批处理/多路复用能力继续扩展到更多 Bridge metadata/control 组合调用，并继续扩展错误分类。
5. 增加 Windows/Linux 真实运行验证、worker supervision 压测、资源上限策略调优和更细粒度 server-push 事件类型。
6. 将 Bridge audio sequence/late/jitter 指标与 WebAudio worker/worklet underflow/overflow 指标打通，并把 Bridge route latency 扩展到端到端 WebAudio 往返测量。
