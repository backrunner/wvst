# WVST Agent Docs

调研日期：2026-06-01

本目录存放 WVST 项目前期的 agent 上下文、技术文档和工程约束。后续实现代码应以这些文档为先验，不把架构假设散落在对话或单个源码文件里。

## 文档索引

- [technical-feasibility.md](technical-feasibility.md)：WebAudio、本地桥接、VST3、WASM 和跨平台可行性调研。
- [requirements.md](requirements.md)：产品目标、功能需求、非功能需求、性能和安全边界。
- [module-design.md](module-design.md)：程序模块、进程架构、Web API、协议和嵌入式接口设计。
- [roadmap.md](roadmap.md)：从 macOS MVP 到跨平台生产可用版本的阶段计划。
- [development-standards.md](development-standards.md)：Rust/TS 实时音频、高性能、测试、代码规模和提交规范。
- [implementation-gap-analysis.md](implementation-gap-analysis.md)：当前实现状态、完整能力差距和下一阶段顺序。
- [skills/wvst-engineering/SKILL.md](skills/wvst-engineering/SKILL.md)：后续 WVST 开发时应加载的本地 skill。

## 当前结论

WVST 的方向可行：Web 侧使用 AudioWorklet + WASM + SharedArrayBuffer 管理 WebAudio 实时缓冲，本地 Bridge Server 负责插件发现、权限、路由和 worker 生命周期，VST3 host worker 以独立进程隔离崩溃插件。

需要明确的边界是：浏览器 AudioWorklet 的 `process()` 不能阻塞等待本地进程返回，也不能把网络/IPC 放到实时音频线程里同步调用。因此 WVST 的低延迟设计必须采用异步桥接、固定延迟补偿、环形缓冲和可观测的 underflow/overflow 处理，而不是承诺零缓冲同步 round-trip。
