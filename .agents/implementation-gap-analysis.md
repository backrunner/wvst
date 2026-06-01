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

## 距离完整能力的主要差距

### 1. 插件实例生命周期

仍缺少：

- `instance.create` / `instance.destroy` / `instance.list`。
- 每个实例的独立 worker、instance id、stream id、状态机和资源释放。
- 同一插件 N 个实例的隔离策略和调度策略。

这是下一阶段最高优先级，因为后续 audio stream、参数和 MIDI 都依赖实例句柄。

### 2. 持久 worker IPC

当前 worker 仍是一次性 CLI 命令模型，适合 scan/probe/metadata，不适合实时处理。

仍缺少：

- worker 常驻模式。
- Bridge 与 worker 的 framed IPC。
- 心跳、超时、kill、restart、stderr 摘要和 crash quarantine。
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

1. 建立 `instance.create` / `instance.destroy` 控制面和 Bridge instance registry。
2. 把 `wvst-host-worker` 扩展为常驻 JSON/frame IPC worker，先支持 fake passthrough instance。
3. 将 Bridge binary echo 改为按 `streamId` 路由到 worker passthrough，形成 WebAudio 到 worker 再返回的端到端闭环。
4. 在 fake passthrough 稳定后，实现 VST3 `createInstance` 和 2-in/2-out effect processing。
5. 增加 worker watchdog、超时 kill、restart metrics 和崩溃 quarantine。
