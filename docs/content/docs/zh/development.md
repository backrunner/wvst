---
title: 开发与验证
description: 理解 workspace 检查、Studio 浏览器回归、真实插件证据，以及可复现的问题报告。
order: 9
---

# 开发与验证

不同检查回答不同问题：类型与单元测试检查实现契约，浏览器 fixture 验证交互和协议链路，真实 VST3 测试验证指定电脑和插件组合。发布结论需要对应的证据。

## 准备工作区

```sh
npm ci
rustup component add rustfmt clippy
rustup target add wasm32-unknown-unknown
```

仓库使用 stable Rust 与 Node.js 22。插件兼容性工作优先在 macOS、匹配 CPU 架构的真实插件上进行。修改前查看 `.agents/implementation-gap-analysis.md` 和 `.agents/development-standards.md`，区分已实现行为与路线图目标。

## 按改动选择检查

以下命令均在仓库根目录执行：

| 命令 | 验证范围 |
| --- | --- |
| `npm run check` | Rust 测试、WASM target 检查、SDK 类型检查/单元测试、示例类型检查。 |
| `cargo fmt --all -- --check` | Rust 格式。 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Rust lint，警告视为错误。 |
| `npm run check:web` | SDK TypeScript。 |
| `npm run test:web` | SDK Vitest 测试。 |
| `npm run build:web:examples` | SDK 和效果器/音源示例的实际打包。 |
| `npm run docs:check` | Svedocs 内容检查和 Svelte 类型检查。 |
| `npm run docs:build` | SDK 与文档生产构建，包含 OG SVG 生成。 |
| `npm run docs:build -- --no-og` | 跳过 OG 生成的文档构建。 |

`npm run check` 不包含文档构建或 Studio 浏览器回归。纯文档修改通常运行 docs check/build 并抽查页面即可；音频协议、资源生命周期和实时路径修改需要对应测试与证据。

## Studio 浏览器回归

先构建，再启动生产预览，避免开发服务器与生产构建同时写入 SvelteKit 生成文件。

终端 A：

```sh
npm run docs:build
npx playwright install chromium
npm --workspace @wvst/docs run preview
```

终端 B：

```sh
npm --workspace @wvst/docs run smoke:studio
```

默认测试地址为 `http://127.0.0.1:4173`。需要其他端口或保存截图时：

```sh
mkdir -p /tmp/wvst-studio-shots
WVST_STUDIO_URL=http://127.0.0.1:4173 \
WVST_STUDIO_SCREENSHOTS=/tmp/wvst-studio-shots \
  npm --workspace @wvst/docs run smoke:studio
```

脚本使用本地协议 fixture，覆盖控制/音频两次握手、音频二进制往返、文件与内置音源、波形、空扫描、挂载失败清理、参数、重排、旁路、移除、重连，以及中英文和手机布局。它不会加载第三方 VST3，也不能替代音质或长时间稳定性测试。

## 真实插件与持续音频证据

先准备有权使用的本机 VST3 fixture。记录插件厂商、名称、版本、类型、CPU 架构、声道配置、采样率和块大小。不要把私有插件二进制放进仓库。

仓库提供以下工具：

| 工具或入口 | 用途 |
| --- | --- |
| `wvst-runtime-matrix` | 按 manifest 批量运行隔离的插件探测，并评估每个 case 的预期结果。 |
| `npm run smoke:bridge:web` | Web SDK 到本地 Bridge/插件的 smoke；先阅读脚本的 fixture 环境变量。 |
| `npm run smoke:browser:web` | 浏览器 WebAudio 路径验证；按脚本配置测试环境。 |
| `npm run evidence:web:bridge-long` | 配置 30 分钟 Bridge smoke 窗口。 |
| `npm run evidence:web:browser-long` | 配置 30 分钟浏览器 smoke 窗口。 |
| `wvst-latency-snapshot` | 把逐块观察转换为延迟快照。 |
| `wvst-stability-budget` | 对延迟、丢帧、错误及可选浏览器/Bridge 指标评估预算。 |
| `wvst-package-evidence` | 检查打包 manifest、验证报告与文件角色等发布证据。 |

示例：先复制并编辑 `.agents/runtime-probe-matrix.example.json`，填入本机真实插件路径与合理 expectations，再运行：

```sh
cargo build -p wvst-host-worker
cargo run -p wvst-testkit --bin wvst-runtime-matrix -- \
  --worker target/debug/wvst-host-worker \
  --manifest /absolute/path/to/local-matrix.json
```

manifest 中的示例路径不是随仓库附带的插件。根据 `.github/workflows/evidence.yml` 配置自托管 runner；普通 CI 的跨平台编译通过，不等于全部平台的第三方插件兼容性通过。

## 提交可复现的问题报告

附上以下信息，能更快定位故障层：

- 当前 commit、macOS/其他系统版本、CPU 架构、浏览器版本。
- 从干净启动到故障的最短步骤，预期与实际结果。
- 插件厂商、名称、版本、class ID、effect/instrument 类型及输入输出声道。
- `AudioContext.sampleRate`、每块帧数、缓冲容量、效果链顺序。
- `diagnose` 结果、相关 `bridge.events()` 或 `instance.runtimeSnapshot()`、计数器的时间区间和增量。
- 声音故障是否在旁路、单实例、重连或更换插件后仍出现。

分享前移除 token 和个人路径等不必要的本机信息。CPU、端到端延迟或稳定性结论应注明测量方法、运行时长和插件组合。

## 贡献约定

原生实现优先 Rust；VST3 ABI 与 unsafe 留在明确边界。Bridge 不加载第三方插件，AudioWorklet 不等待原生处理，不加入 JSON、网络、日志或阻塞操作。新增协议与生命周期行为应附对应测试，并同步两个语言的公共文档。

文档站主题、内容路径和 Logo 维护说明见仓库 `docs/README.md` 与 `docs/brand.md`。
