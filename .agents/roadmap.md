# WVST Roadmap

调研日期：2026-06-01

## Phase 0：文档与技术验证准备

目标：

- 固化可行性、需求、模块设计和工程规范。
- 形成 WVST 开发 skill。
- 明确 macOS MVP 范围和非目标。

完成标准：

- `.agents` 文档可作为后续实现依据。
- 提交信息遵循 `docs(agents): ...`。

## Phase 1：仓库骨架与协议原型

目标：

- 初始化 Rust workspace 和 TS/Rollup package。
- 建立 `wvst-core`、`wvst-protocol`、`wvst-ringbuf`、`wvst-testkit`。
- 实现控制面 hello/version negotiation。
- 实现音频二进制帧编码/解码和 fixtures。
- 建立 CI：fmt、clippy、test、tsc、rollup build。

完成标准：

- Rust/TS schema fixtures 双向一致。
- fake loopback processor 可通过 Bridge echo 音频帧。
- 有 microbench 覆盖音频帧编码和 ring buffer。

## Phase 2：Web SDK 与本地 Bridge MVP

目标：

- `wvst-bridge-server` 提供 localhost WebSocket。
- `packages/wvst-web` 提供 `WVSTClient.connect()`。
- AudioWorklet + DedicatedWorker + SAB 跑通本地 loopback。
- 实现 pairing token、origin allowlist、基础 metrics。

完成标准：

- Web 示例可把麦克风或 oscillator 送入 fake processor 并输出。
- underflow/overflow 可观测。
- 无 SAB 时明确报错。

## Phase 3：macOS VST3 扫描与 headless host

目标：

- macOS VST3 路径扫描和缓存。
- `wvst-host-worker` 独立进程。
- `wvst-vst3-host` 加载 VST3 factory，读取 metadata。
- 支持一个 2-in/2-out effect 插件实例的 headless processing。

完成标准：

- Web 端可列出插件名称、厂商、类别和参数。
- Bridge Server 不加载插件，插件只在 worker 进程中加载。
- 插件崩溃不会杀死 Bridge Server。

## Phase 4：低延迟音频处理与参数控制

目标：

- WebAudio 输入通过 Bridge 送入 VST3 effect，再回到 AudioWorklet 输出。
- 支持参数 set/get、begin/perform/end edit。
- 支持 plugin latency 上报和 bridge latency 补偿。
- 建立 latency harness 和长时间稳定性测试。

完成标准：

- 48 kHz、128 frame、2-in/2-out 在默认缓冲下稳定运行 30 分钟。
- 报告 p50/p95/p99 round-trip 和 drop 指标。
- 参数自动化在 block 内 offset 生效。

## Phase 5：音源 VST、MIDI 与 state

目标：

- 支持 instrument 插件无输入生成音频。
- 支持 MIDI/note event、CC、pitch bend、aftertouch。
- 支持 state get/set 和参数快照。
- 支持 program list metadata。

完成标准：

- Web 示例可用虚拟键盘或 Web MIDI 触发 VST instrument。
- note event 带 sample offset。
- 刷新页面后可恢复插件 state。

## Phase 6：稳定性、安全与打包

目标：

- 完善 worker watchdog、quarantine、restart policy。
- 完善 macOS 安装、自动启动、日志路径、权限提示。
- 增加本地 CLI：scan、list、diagnose、reset-token。
- 增加安全 review：origin、token、PNA/CORS、路径授权。

完成标准：

- 恶意 origin 不能调用插件 API。
- 连续崩溃插件不会进入重启风暴。
- 用户能撤销已授权 origin。

## Phase 7：跨平台扩展

目标：

- Windows VST3 扫描、worker、process supervision。
- Linux VST3 扫描、worker、process supervision。
- 平台差异文档和测试矩阵。
- 评估 WebTransport 数据面。

完成标准：

- Windows/macOS/Linux 均能完成 plugin list 和 basic effect processing。
- 协议不因平台不同而变化。
- 平台 worker 崩溃均可恢复。

## Phase 8：生产化与生态

目标：

- API semver 规则。
- 插件兼容性数据库。
- 性能 profile 指南。
- 更多示例：效果器链、instrument、generic UI、embedded app。
- 可选浏览器扩展用于启动/配对体验。

完成标准：

- 发布 `wvst-bridge`、Rust crates、`@wvst/web`。
- 文档包含安全、部署、浏览器 header 和 troubleshooting。
- 公开 benchmark 数据和已知限制。

