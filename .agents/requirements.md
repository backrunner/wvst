# WVST 需求文档

调研日期：2026-06-01

## 目标

WVST 是一个开源项目，用 Rust 构建 WebAudio 与本地 VST 插件之间的低延迟桥梁，让 Web 应用可以发现、挂载、控制并处理本机 VST 插件。Web 是主要 UI 承载方，本地 Bridge Server 负责 VST 插件运行、隔离和音频处理。

## 角色

- Web 开发者：在网页中使用 WVST SDK，把 VST 插件作为 WebAudio graph 的一部分。
- 终端用户：安装 Bridge Server，授权网页访问本机 VST 插件。
- 应用开发者：把 Bridge Runtime 嵌入自己的桌面应用中，而不是只运行独立 daemon。
- 插件开发者/测试者：用 WVST 验证 VST3 插件在 WebAudio 环境中的处理和 metadata 暴露。

## 范围

MVP 范围：

- macOS 优先。
- VST3 优先。
- Web SDK 支持 TypeScript + Rust WASM。
- 本地 Bridge Server 支持独立运行。
- 支持效果器和音源 VST。
- 支持同一插件的多个独立实例。
- 支持插件崩溃/ANR 隔离和可恢复错误。

暂不作为 MVP：

- 原生 VST UI 嵌入浏览器页面。
- VST2/AU/AAX/CLAP。
- 分布式网络远端 VST。
- DAW 工程文件兼容。
- 多插件机架、复杂 routing graph。WVST 只暴露单实例或由 Web 侧自行组织 graph。

## 功能需求

### Web SDK

- FR-WEB-001：提供 `WVSTClient.connect()` 建立与本地 Bridge Server 的会话。
- FR-WEB-002：提供本地 Bridge 发现和配对流程，包括 token、origin 校验、版本协商和错误提示。
- FR-WEB-003：提供 `listPlugins()`、`getPlugin(id)`、`scanPlugins()` 等 metadata API。
- FR-WEB-004：提供 `createInstance(pluginId, options)` 创建 VST 实例。
- FR-WEB-005：提供 `createAudioNode(instance, options)` 或等价 API，把实例挂载为 WebAudio graph 中的节点。
- FR-WEB-006：支持获取插件名称、厂商、版本、类别、I/O bus、参数、单位、program list、latency、tail、是否为 instrument/effect。
- FR-WEB-007：支持参数读写、参数文本展示、自动化事件调度和 sample-offset 参数变化。
- FR-WEB-008：支持 MIDI/note 输入事件，包括 note on/off、pitch bend、CC、aftertouch，以及可扩展 VST3 note expression。
- FR-WEB-009：暴露连接状态、实时状态、当前延迟、drop/underflow/overflow、插件崩溃和恢复事件。
- FR-WEB-010：当 SAB 不可用时明确失败或进入高延迟 fallback，不静默伪装为低延迟模式。

### Bridge Server

- FR-BRIDGE-001：使用 Rust 实现，支持独立可执行文件。
- FR-BRIDGE-002：提供可嵌入 runtime API，允许桌面应用内置 Bridge Server。
- FR-BRIDGE-003：提供控制面 API：会话、权限、插件扫描、实例生命周期、参数/state、诊断。
- FR-BRIDGE-004：提供音频数据面：低开销二进制帧、sequence、timestamp、format negotiation、backpressure。
- FR-BRIDGE-005：本身不直接加载第三方插件；通过 host worker 进程隔离插件。
- FR-BRIDGE-006：支持同一个插件创建 N 个独立实例，每个实例拥有独立 state、参数、MIDI 队列和音频 stream。
- FR-BRIDGE-007：维护插件扫描缓存，并能检测插件文件变化。
- FR-BRIDGE-008：支持 macOS 默认 VST3 路径和用户自定义路径；Windows/Linux 路径作为架构预留并在后续实现。
- FR-BRIDGE-009：支持 origin allowlist、用户授权、短期配对码、长期 token 撤销。

### VST Host Worker

- FR-HOST-001：加载 VST3 插件并枚举 factory class。
- FR-HOST-002：遵循 VST3 component/controller lifecycle。
- FR-HOST-003：配置 sample rate、max block size、process mode、bus arrangement 和 active buses。
- FR-HOST-004：支持 32-bit float audio buffers，后续可扩展 64-bit。
- FR-HOST-005：支持 effect 插件的输入/输出处理。
- FR-HOST-006：支持 instrument 插件无音频输入或可选输入情况下由 MIDI/note event 生成音频。
- FR-HOST-007：支持参数变化、sample-accurate automation、program/state get/set。
- FR-HOST-008：查询并上报 plugin latency、tail、silence flags 和 bus 信息。
- FR-HOST-009：遇到插件崩溃、panic、超时、非法返回值时向 Bridge Server 上报结构化错误。
- FR-HOST-010：实时处理线程不得做文件、网络、UI、阻塞锁和动态分配。

### Web UI 能力

- FR-UI-001：Web 端能基于 metadata 构建通用参数 UI。
- FR-UI-002：参数值必须支持 normalized value 与显示文本转换。
- FR-UI-003：插件 native editor 作为后续能力，只能以本地窗口或远程 UI 表示方式探索，不承诺浏览器内嵌。

## 非功能需求

### 性能

- NFR-PERF-001：默认目标 48 kHz、128 frame quantum、2-in/2-out effect 场景稳定运行。
- NFR-PERF-002：MVP 默认桥接缓冲 4-8 quantum，可配置最小 2 quantum 并报告风险。
- NFR-PERF-003：音频 hot path 不使用 JSON、不分配、不复制超过必要次数。
- NFR-PERF-004：所有音频帧必须有 sequence，过期帧可丢弃或标记，不能阻塞 realtime path。
- NFR-PERF-005：Bridge Server 需要暴露 p50/p95/p99 round-trip、jitter、drop、CPU、worker restart 指标。

### 稳定性

- NFR-REL-001：插件崩溃不得导致 Bridge Server 或浏览器崩溃。
- NFR-REL-002：worker 心跳超时后必须可 kill/restart，并通知 Web SDK。
- NFR-REL-003：连续崩溃的插件或实例进入 quarantine，防止自动重启风暴。
- NFR-REL-004：错误恢复策略必须明确：`mute`、`bypass`、`destroy` 或 `restart`。

### 安全

- NFR-SEC-001：Bridge Server 只监听 loopback，默认不监听局域网。
- NFR-SEC-002：所有控制 API 必须要求会话 token。
- NFR-SEC-003：Bridge Server 不得对任意 origin 开放强权限 API。
- NFR-SEC-004：本地扫描路径、插件启动和文件读写必须最小权限。
- NFR-SEC-005：Web SDK 要向开发者暴露安全前置条件：secure context、cross-origin isolation、localhost/PNA 限制。

### 跨平台

- NFR-PLAT-001：代码结构从第一天隔离平台实现。
- NFR-PLAT-002：macOS MVP 完成后扩展 Windows，再扩展 Linux。
- NFR-PLAT-003：协议、缓存和配置必须跨平台稳定，不依赖平台路径分隔或 native struct layout。

### 开源与许可

- NFR-LIC-001：项目默认使用与 VST3 SDK MIT 兼容的开源许可。
- NFR-LIC-002：不得引入会强制污染整个项目许可的依赖，除非经过明确 ADR 批准。
- NFR-LIC-003：VST 商标和 Logo 使用必须遵守 Steinberg 使用规则。

## 开放问题

- 首个公开版本是否要求浏览器扩展来改善本地 Bridge 配对体验？
- WebTransport 是否在 MVP 中实验性支持，还是等 WebSocket 数据面稳定后再引入？
- MVP 是否允许 trusted mode 将多个实例放入同一 worker 进程？
- 是否需要在首版支持插件 state 的持久化格式导入/导出？
- 是否需要提供 Web MIDI 到 VST note event 的内置 adapter？

