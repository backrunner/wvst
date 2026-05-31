# WVST 开发规范

调研日期：2026-06-01

## 总原则

- Rust-first：本地、协议、扫描、VST host、测试工具都使用 Rust。
- Web 侧只在必要处使用 TypeScript；高频协议和 ring-buffer 逻辑优先由 Rust 编译到 WASM 复用。
- 实时音频路径优先级高于 API 便利性。
- 小模块、窄接口、可测量性能，不写巨型单文件。
- 所有跨进程、跨语言、跨平台边界必须有版本、测试 fixtures 和错误码。

## 文件与模块规模

- 单个源码文件目标小于 400 行，超过 600 行必须拆分或写明原因。
- 单个函数目标小于 80 行，复杂函数必须拆出状态机、parser 或 helper。
- 一个模块只承担一个明确职责。
- `unsafe` 只能出现在明确命名的边界模块，如 `ffi`、`com`、`loader`，并附安全说明。
- 不允许把 protocol、transport、business logic 和 platform code 混在同一个文件。

## Rust 规范

- 使用 stable Rust，workspace 统一 `rust-toolchain.toml`。
- `cargo fmt`、`cargo clippy --all-targets --all-features -D warnings` 必须通过。
- 错误使用 `thiserror`/结构化错误，不用裸字符串横跨模块。
- 日志使用 `tracing`，实时线程不得高频日志。
- 公共类型尽量 `#[non_exhaustive]`，协议类型显式 version。
- 禁止在 hot path 使用无界 channel。
- 不在实时线程使用 `Mutex`、`RwLock`、文件 IO、网络 IO、heap allocation、blocking syscall。
- 需要共享状态时优先使用 SPSC ring、预分配 buffer、atomic cursor、double-buffer snapshot。
- 所有 `unsafe` 块必须说明：
  - 指针/生命周期来源；
  - 线程约束；
  - 对齐和长度约束；
  - 谁负责释放；
  - panic/FFI unwind 如何处理。

## Web/TypeScript 规范

- TypeScript 开启 `strict`。
- Rollup 输出 ESM，避免把 Node-only polyfill 打进浏览器包。
- AudioWorklet 文件独立 bundle，不能依赖 DOM-only API。
- `AudioWorkletProcessor.process()` 内禁止 Promise、fetch/WebSocket、JSON、console 高频日志和大对象创建。
- SAB 不可用时必须给出明确错误与修复提示。
- 所有 public API 都要有 `.d.ts` 类型、单元测试和最小示例。

## 协议规范

- 控制面每条消息包含 protocol version。
- 音频面使用二进制格式，不使用 JSON。
- 字节序固定 little-endian。
- 所有 frame 包含 `stream_id`、`sequence`、`frames`、`channels`、`sample_rate`。
- 不依赖 Rust struct 内存布局直接跨语言传输。
- schema 更新必须增加 fixture，旧客户端错误要可诊断。

## 实时音频规范

- 音频线程只做确定性、有上界的工作。
- 每个实例启动时预分配最大 block、channel、event buffer。
- 参数自动化和 MIDI event 在进入音频线程前排序。
- worker 与 Bridge 的数据交换必须有 backpressure 策略。
- underflow 策略只能是显式配置：`mute`、`dry`、`hold`，默认 `mute`。
- overflow 策略默认丢弃最旧过期帧，并增加指标。
- 插件报告 latency 改变时必须重新发布给 Web SDK。

## VST3 Host 规范

- Bridge Server 禁止直接加载 VST3。
- Host worker 必须隔离插件崩溃。
- VST3 lifecycle 必须有状态机测试。
- `setProcessing(true/false)` 和 `process()` 路径不得分配。
- plugin state 作为 opaque bytes 处理。
- 不承诺 native editor 嵌入 Web；generic parameter UI 是默认能力。

## 安全规范

- 默认只监听 loopback。
- 控制面必须鉴权。
- origin allowlist 精确匹配，不使用 wildcard 授予强权限。
- 配对 token 可撤销、可过期。
- 插件路径扫描需要显式授权。
- 日志不得记录原始音频内容，除非用户在诊断模式显式开启。

## 测试规范

每个 crate 至少包含：

- 单元测试：纯逻辑、parser、state machine。
- 集成测试：control API、worker lifecycle、scanner fixtures。
- 属性/模糊测试：协议 decode、ring cursor、frame boundary。
- 性能测试：音频 frame encode/decode、ring buffer、process dispatch。

推荐工具：

- `cargo test --all`
- `cargo clippy --all-targets --all-features -D warnings`
- `cargo fmt --check`
- `cargo deny check`
- `cargo nextest run` 后续引入
- `criterion` 用于 microbench
- `wasm-bindgen-test` 用于 WASM 包
- `vitest` 或等价工具用于 TS SDK

## 性能验收

必须持续记录：

- round-trip p50/p95/p99
- jitter p95/p99
- underflow/overflow count
- late frame dropped
- process CPU time
- Bridge CPU/memory
- worker restart count

任何影响 hot path 的 PR 必须包含 benchmark 前后对比或说明为何不影响。

## 代码异味红线

- 单文件塞入多个抽象层。
- 以字符串拼接解析二进制协议。
- `unwrap()`/`expect()` 出现在长期运行服务路径。
- 对插件崩溃只打印日志不改变状态。
- 音频线程里分配、锁、网络、文件、UI 调用。
- 跨平台代码到处 `#[cfg]`，而不是收敛在 platform backend。
- Web SDK 隐藏低延迟失败，导致开发者以为仍处于 realtime 模式。

## 提交规范

提交身份必须使用：

```text
BackRunner <dev@backrunner.top>
```

提交信息格式：

```text
type(scope): subject
```

示例：

```text
docs(agents): add initial feasibility and architecture docs
feat(protocol): add binary audio frame header
fix(bridge): reject unauthenticated origin
perf(ringbuf): reduce audio frame copies
test(vst3): cover processor lifecycle transitions
```

允许的 `type`：

- `feat`
- `fix`
- `docs`
- `refactor`
- `perf`
- `test`
- `chore`
- `ci`

本地提交命令示例：

```sh
git -c user.name="BackRunner" -c user.email="dev@backrunner.top" commit -m "docs(agents): add initial feasibility and architecture docs"
```

## PR 检查清单

- 代码是否保持模块边界清晰？
- 是否新增或更新测试？
- 是否影响音频 hot path？
- 是否引入新的 allocation/lock/copy？
- 是否有跨平台路径或进程行为差异？
- 是否更新 `.agents` 中相关文档？
- 是否遵循提交身份和 message 格式？

