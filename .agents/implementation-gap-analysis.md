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
- Bridge worker supervisor 已校验 `worker.hello` 中的 `ipcVersion`、`instanceLifecycle` 和 `binaryAudioProcess` capability，避免 Bridge 与不兼容 worker 继续创建实例。
- Bridge 二进制音频帧已能按 `streamId` 路由到对应 worker 的独立二进制 audio IPC，并回传 worker 处理后的 F32 frame；未匹配实例或非法帧暂时保留 echo fallback。
- Bridge metrics 已区分二进制帧总量、成功路由音频帧、fallback echo 和音频路由失败，便于后续接入 drop/late/underflow/overflow 统计。
- Bridge/Web SDK 已提供 `stream.open` / `stream.close` 控制 API，实例记录包含 `streamState`，Bridge 只将 open stream 的音频帧路由到 worker。
- 对于已知 stream 的关闭或处理失败场景，Bridge 会返回带 `silence` / `end-of-stream` / `process-error` flag 的诊断静音音频帧，避免把异常伪装成正常 echo。

## 距离完整能力的主要差距

### 1. 插件实例运行态生命周期

仍缺少：

- `ready` 之后的 `processing`、`stopping` 等更细生命周期，以及对启动中状态的显式暴露。
- 每个实例的独立 worker 进程已具备原型，并支持手动 restart；仍缺少自动 restart 状态机、崩溃事件推送和策略化资源回收。
- 同一插件 N 个实例的实际 worker 隔离策略和调度策略。

这是下一阶段最高优先级，因为当前实例句柄还没有绑定常驻 worker 和真实 VST 对象。

### 2. 持久 worker IPC

当前 Bridge 已能为实例启动并绑定一个常驻 worker 进程，控制面仍是 JSON-line 原型，音频面已有独立二进制 IPC。

仍缺少：

- 更完整的 Bridge worker supervisor 生命周期管理，包括自动 restart policy、主动 crash event 推送和 quarantine 解除策略。
- 正式 framed control IPC，替换当前 JSON-line 控制面原型。
- 超时后的全链路 kill/wait 审计、restart 指标和 crash quarantine 解除策略。
- 更完整的 worker capability negotiation，包括按数据面、MIDI、参数自动化和诊断能力分层协商。

### 3. 真实 VST3 component/controller lifecycle

当前只完成 factory info 读取，尚未创建真实 VST3 component/controller。

仍缺少：

- `IPluginFactory::createInstance` ABI。
- component/controller 初始化、bus arrangement、sample rate、max block size。
- `setProcessing`、`process`、latency/tail 查询。
- 参数、state、program list、unit metadata。

### 4. 低延迟音频数据面

当前 Bridge 二进制帧已具备按 `streamId` 到 worker 二进制 audio IPC 的 passthrough 原型路由，但仍不是最终低延迟数据面。

仍缺少：

- stream open/close 已有首版控制 API；仍缺少 end-of-stream 帧语义、close 后 drain 策略和 WebAudio 端自动重开策略。
- Web Worker 从 SAB 取音频块并编码发送。
- Bridge 到 worker 的二进制 audio IPC 已具备首版；Bridge/Web 二进制诊断帧已有基础 flags，仍缺少共享内存/预分配 buffer 和背压语义。
- worker 到真实 VST `process()` 的预分配 buffer 路径。
- late/drop/underflow/overflow 策略和 p50/p95/p99 指标；当前只有 route/fallback/failure 计数，还没有时延分位数。

### 5. MIDI 与音源 VST

仍缺少：

- MIDI/note event schema。
- sample offset 保留。
- instrument 无输入生成音频路径。
- Web MIDI adapter 和虚拟键盘示例。

### 6. 嵌入式 runtime 与打包

仍缺少：

- `wvst-embed` 或等价可嵌入 API。
- macOS 安装、启动、授权、日志和诊断命令。
- Windows/Linux worker supervision backend。

## 建议下一阶段

1. 给 Bridge worker supervisor 增加自动 restart policy、quarantine 解除策略和 worker crash 事件回传。
2. 把 worker JSON-line 控制 IPC 抽象为可替换 framed control IPC，并扩展 capability negotiation。
3. 为 audio IPC 增加错误帧语义、stream open/close 和 backpressure/late-frame 指标。
4. 在 fake passthrough 稳定后，实现 VST3 `createInstance` 和 2-in/2-out effect processing。
5. 增加 worker watchdog、超时 kill、restart metrics 和崩溃 quarantine。
