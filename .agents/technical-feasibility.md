# WVST 技术可行性调研

调研日期：2026-06-01

## 结论摘要

WVST 可以实现，但必须把目标定义为“可测量、可补偿、低抖动的低延迟桥接”，而不是“浏览器音频线程同步调用本地 VST”。浏览器不能加载本地 VST，也不能把 native shared memory 直接映射给 WebAudio；WebAudio 的实时线程必须准时返回。因此架构上应使用：

1. Web 侧 `AudioWorkletProcessor` 只做实时安全的 ring-buffer 读写、混音、静音/旁路和指标计数。
2. Web 侧 `DedicatedWorker` 负责本地 Bridge Server 连接和二进制音频帧传输。
3. Web SDK 内部使用 Rust 编译到 WASM 实现协议、环形缓冲游标和少量高频路径。
4. 本地 Bridge Server 使用 Rust，作为独立服务或嵌入式 runtime。
5. 每个 VST 实例或隔离组在独立 host worker 进程中运行，避免插件崩溃拖垮 Bridge Server。

## 官方资料依据

- MDN AudioWorklet 文档说明，`AudioWorkletProcessor.process()` 运行在 Web Audio 的块处理路径中；当前音频块通常是 128 帧，但未来不应硬编码该值。来源：[MDN AudioWorkletProcessor.process](https://developer.mozilla.org/en-US/docs/Web/API/AudioWorkletProcessor/process)、[MDN Using AudioWorklet](https://developer.mozilla.org/en-US/docs/Web/API/Web_Audio_API/Using_AudioWorklet)。
- `SharedArrayBuffer` 需要安全上下文和跨源隔离，通常需要 COOP/COEP；这会影响第三方网页集成方式。来源：[MDN SharedArrayBuffer](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer)、[MDN COEP](https://developer.mozilla.org/docs/Web/HTTP/Reference/Headers/Cross-Origin-Embedder-Policy)。
- WebSocket 支持二进制数据并可在 Web Worker 中使用；WebTransport 提供 streams 和 UDP-like datagrams，但需要安全上下文，浏览器支持和本地证书/HTTP3 部署会影响采用节奏。来源：[MDN WebSocket.binaryType](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/binaryType)、[MDN WebTransport API](https://developer.mozilla.org/en-US/docs/Web/API/WebTransport_API)。
- Chrome Native Messaging 是 JSON + 32-bit 长度前缀协议，并限制 native host 发给浏览器的单条消息大小；它适合启动、配对和控制面，不适合作为持续音频流。来源：[Chrome Native Messaging](https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging)。
- 公共网页访问 localhost/私有网络端点会受到 Private Network Access/CORS 约束；Bridge Server 必须显式处理预检、origin 和本地授权。来源：[Chrome Private Network Access preflights](https://developer.chrome.com/blog/private-network-access-preflight)。
- VST3 SDK 从 3.8 起是 MIT 许可，官方 SDK 支持 Windows/macOS/Linux，并包含 host 开发所需接口。来源：[VST 3 Licensing](https://steinbergmedia.github.io/vst3_dev_portal/pages/VST%2B3%2BLicensing/Index.html)、[steinbergmedia/vst3sdk](https://github.com/steinbergmedia/vst3sdk)、[Steinberg VST SDK](https://www.steinberg.net/developers/vstsdk/)。
- VST3 host 需要遵循音频处理生命周期，`process()` 在 audio thread 调用，`setProcessing()` 可能从实时线程调用且要求 lock-free/无分配。来源：[VST3 Audio Processor Call Sequence](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/Workflow%2BDiagrams/Audio%2BProcessor%2BCall%2BSequence.html)、[VST3 Processing FAQ](https://steinbergmedia.github.io/vst3_dev_portal/pages/FAQ/Processing.html)。
- VST3 `ProcessData` 包含音频总线、参数变化和事件列表；这覆盖效果器、音源、MIDI/note 事件和自动化的基础能力。来源：[VST3 ProcessData](https://steinbergmedia.github.io/vst3_doc/vstinterfaces/structSteinberg_1_1Vst_1_1ProcessData.html)、[VST3 Parameters and Automation](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/Parameters%2BAutomation/Index.html)。

## WebAudio 与本地桥接

WebAudio 适合作为浏览器内实时音频入口，但它不适合直接执行异步 IPC。`AudioWorkletProcessor.process()` 的职责应保持极小：读取输入样本、写入输出样本、处理已经到达的控制事件、维护 ring-buffer 指针和丢包指标。

推荐 Web 侧分层：

- `AudioWorkletProcessor`：实时线程，只访问预分配 TypedArray/SAB，不发起网络请求，不等待 Promise，不写 console，不做 JSON。
- `DedicatedWorker`：传输线程，持有 WebSocket 或 WebTransport 连接，与 worklet 通过 SAB 交换音频帧。
- `Main thread`：用户 API、权限提示、插件选择、UI 状态、AudioNode 创建。
- WASM：实现协议编码、ring cursor、resampling/format conversion 中可预测的 hot path。

最低延迟预算的现实约束：

- 128 帧在 48 kHz 下约 2.67 ms，在 44.1 kHz 下约 2.90 ms。
- 一次 Web -> worker -> localhost -> host worker -> VST -> localhost -> worker -> worklet 的完整 round-trip 通常至少需要多个 render quantum 的缓冲。
- MVP 应先设目标：默认 4-8 quantum 安全缓冲，可调到 2-4 quantum；把实际 round-trip、underflow、overflow、jitter 暴露给开发者。
- 对效果器，输出天然会是延迟补偿后的历史输入处理结果；对音源 VST，MIDI 事件需要带 sample offset 或 block timestamp。

## 传输方案评估

| 方案 | 用途 | 优点 | 风险 | 建议 |
| --- | --- | --- | --- | --- |
| localhost WebSocket | MVP 音频与控制面 | 实现简单、浏览器覆盖好、Worker 可用 | TCP head-of-line、不能丢弃过期帧、抖动需要自控 | macOS MVP 首选 |
| WebTransport | 后续音频数据面 | 支持可靠流和不可靠 datagram，更适合实时帧 | HTTPS/HTTP3/证书、兼容性和部署复杂度 | 作为 Phase 4+ 优化 |
| Native Messaging | 启动/配对/控制 | 可由浏览器扩展启动本地 host，来源明确 | JSON、消息大小、stdio，不适合高频音频 | 只用于 extension 模式控制面 |
| HTTP fetch/stream | 元数据、下载、诊断 | 调试方便 | 实时性不足 | 非音频 API 可用 |
| Browser direct shared memory with native | 音频数据面 | 理论延迟最低 | 标准 Web 页面不可直接映射 native shared memory | 不作为 Web MVP 假设 |

## VST3 Host 可行性

VST3 是正确的优先目标。VST2 已经不适合作为新开源项目的基础；VST3 的 MIT 许可降低了开源分发风险，且官方 SDK 覆盖三大桌面平台。

Rust 实现路径：

- 以 Rust 为主体实现 Bridge Server、协议、worker 生命周期、扫描缓存、音频调度和测试工具。
- VST3 ABI 边界单独封装在 `wvst-vst3-host` crate，所有 `unsafe` 局限在该 crate 的 `ffi`/`com`/`factory` 模块。
- 先验证现有 Rust VST3 bindings 是否满足 host 需求；若不足，补写最小 host 侧接口绑定，而不是把 C++ SDK 大面积引入业务层。
- VST3 worker 进程可以动态加载 `.vst3` bundle/dll/so，并实现 component/controller 初始化、bus arrangement、sample rate、max block、parameter/event queues、state/preset 和 latency/tail 查询。

## 崩溃、ANR 与隔离

本地 VST 插件是不可信 native code。只要插件与 Bridge Server 同进程，崩溃就会拖垮整个会话；如果插件在实时线程阻塞，也会造成音频 ANR。因此必须采用多进程隔离：

- Bridge Server 不直接加载第三方 VST。
- Host worker 独立进程加载插件；macOS MVP 默认每个插件实例一个 worker。
- Bridge Server 维护心跳、处理超时、启动失败、崩溃计数和 quarantine。
- 音频 stream 遇到 worker 异常时，Web 侧立即进入可预测的 `mute` 或 `bypass` 策略，并发送 `processor:error` 事件。
- 后续可增加 trusted mode：同一插件多个实例共享 worker，换取更低资源占用，但默认不启用。

## 跨平台可行性

Roadmap 优先 macOS 是合理的，因为 macOS 的 VST3 bundle 格式、插件路径和签名/隔离问题适合先收敛。Windows/Linux 不应在架构上后补：

- 插件定位抽象必须从第一天支持 platform backend。
- 进程启动、权限、信号、超时和 crash reason 必须通过 `wvst-process` 抽象。
- 音频和控制协议必须字节序明确，不能依赖平台内存布局。
- VST3 bundle 格式在 Windows/macOS/Linux 结构不同，扫描器需要按平台解析。来源：[VST3 Plug-in Format Structure](https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical%2BDocumentation/Locations%2BFormat/Plugin%2BFormat.html)。

## 主要风险

1. 延迟风险：浏览器和本地进程之间没有硬实时保证。缓解：固定延迟缓冲、指标暴露、延迟补偿、WebTransport 实验分支。
2. 浏览器安全策略风险：SAB 需要跨源隔离，localhost 访问受 PNA/CORS 影响。缓解：明确 SDK 集成要求、origin allowlist、extension/bootstrap 模式。
3. VST host 复杂度风险：VST3 lifecycle、bus、automation、state、UI 都复杂。缓解：先支持 headless processing + generic parameter UI，native editor 后置。
4. 插件稳定性风险：第三方插件可能崩溃、卡死、泄漏或破坏实时线程。缓解：进程隔离、watchdog、黑名单、诊断报告。
5. Rust VST3 host 生态风险：现有 crate 可能覆盖不足。缓解：把 ABI 层隔离，必要时实现最小绑定。

## 可行性判定

建议进入原型阶段，但必须把成功标准定义为可测量：

- macOS 上能扫描 VST3，读取名称、厂商、类别、参数和 bus。
- Web SDK 能列出插件、创建实例、挂载到 WebAudio graph。
- 2-in/2-out effect 插件在 48 kHz 下稳定处理，默认缓冲无明显 drop。
- 音源 VST 可接收 MIDI/note event 并返回音频。
- 插件崩溃不会导致 Bridge Server 或网页崩溃。
- SDK 暴露真实 latency、jitter、drop、worker restart 和 plugin latency。

