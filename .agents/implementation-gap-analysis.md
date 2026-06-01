# WVST Implementation Gap Analysis

更新日期：2026-06-01

## 当前已完成能力

- Rust workspace、TS/Rollup Web SDK、协议帧和基础测试已建立。
- Bridge Server 已提供 localhost WebSocket、hello/version negotiation、origin/token 校验、metrics 和二进制帧 echo。
- Web SDK 已提供 `WVSTClient.connect()`、低延迟前置条件检查、插件 scan/list API、worker/worklet loopback 基础模块。
- macOS VST3 scanner 已支持默认路径和自定义路径，能解析 `moduleinfo.json` 并回退到 bundle 名称。
- `wvst-host-worker` 已作为独立进程入口，支持 VST3 bundle 描述、module symbol probe、macOS factory info 读取和 passthrough probe。
- Bridge 已通过 `plugin.factoryInfo` 将 factory metadata 请求路由到隔离 worker，Bridge 自身不加载第三方 VST。
- Bridge 控制面已增加连接级会话门禁，除 `bridge.hello` 外的 API 需要先完成授权 hello。
- Bridge/Web SDK 已提供 `instance.create` / `instance.list` / `instance.destroy` 控制面，能为已扫描插件分配独立 `instanceId` 与 `streamId`。
- `wvst-host-worker serve` 已提供常驻 JSON-line IPC 原型，支持 worker hello、fake passthrough instance create/destroy 和 debug 小块处理。
- Bridge `instance.create` 已能启动并绑定 `wvst-host-worker serve`，成功后实例进入 `ready` / `ready` 状态。
- Bridge worker supervisor 已加入 stderr 摘要、启动失败计数和基础 quarantine，避免同一故障插件无限重启。

## 距离完整能力的主要差距

### 1. 插件实例运行态生命周期

仍缺少：

- `allocated` 之后的 worker-backed 状态迁移，例如 `starting`、`ready`、`processing`、`failed`。
- 每个实例的独立 worker 进程、资源释放和 crash/restart 状态机。
- 同一插件 N 个实例的实际 worker 隔离策略和调度策略。

这是下一阶段最高优先级，因为当前实例句柄还没有绑定常驻 worker 和真实 VST 对象。

### 2. 持久 worker IPC

当前 Bridge 已能为实例启动并绑定一个常驻 worker 进程，但 IPC 仍是 JSON-line 原型。

仍缺少：

- 更完整的 Bridge worker supervisor 生命周期管理，包括 heartbeat、restart policy 和崩溃状态回传。
- 正式 framed IPC，替换当前 JSON-line 原型。
- 超时后的全链路 kill/wait 审计、restart 指标和 crash quarantine 解除策略。
- worker capability negotiation，确保 Bridge/Web/worker 协议匹配。

### 3. 真实 VST3 component/controller lifecycle

当前只完成 factory info 读取，尚未创建真实 VST3 component/controller。

仍缺少：

- `IPluginFactory::createInstance` ABI。
- component/controller 初始化、bus arrangement、sample rate、max block size。
- `setProcessing`、`process`、latency/tail 查询。
- 参数、state、program list、unit metadata。

### 4. 低延迟音频数据面

当前 Bridge 二进制帧仍是 echo 原型。

仍缺少：

- stream open/close。
- Web Worker 从 SAB 取音频块并编码发送。
- Bridge 到 worker 的 audio IPC。
- worker 到 VST `process()` 的预分配 buffer 路径。
- late/drop/underflow/overflow 策略和 p50/p95/p99 指标。

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

1. 给 Bridge worker supervisor 增加 heartbeat、restart policy、quarantine 解除策略和 worker crash 事件回传。
2. 把 worker JSON-line IPC 抽象为可替换 framed IPC，并加入 capability negotiation。
3. 将 Bridge binary echo 改为按 `streamId` 路由到 worker passthrough，形成 WebAudio 到 worker 再返回的端到端闭环。
4. 在 fake passthrough 稳定后，实现 VST3 `createInstance` 和 2-in/2-out effect processing。
5. 增加 worker watchdog、超时 kill、restart metrics 和崩溃 quarantine。
