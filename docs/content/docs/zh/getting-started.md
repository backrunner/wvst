---
title: 快速开始
description: 从源码启动 Bridge，在 Live Studio 跑通第一个本地 VST3 效果器，并确认每一层工作正常。
order: 2
---

# 快速开始

本指南先跑通仓库自带的 Live Studio。你需要同一台电脑上的浏览器、Rust Bridge、host worker 和 VST3 效果器。网页部署到远程服务器后，Bridge 仍运行在使用者电脑上。

## 准备环境

| 项目 | 要求与检查方法 |
| --- | --- |
| Node.js | 使用 22，运行 `node --version` 检查。 |
| Rust | 使用仓库 `rust-toolchain.toml` 的 stable 工具链；`rustc --version` 可查看当前版本。 |
| macOS 编译工具 | `xcode-select -p` 应输出开发工具路径；未安装时运行 `xcode-select --install`。 |
| 浏览器 | 优先当前桌面 Chromium；需要 AudioWorklet、安全上下文和跨源隔离。 |
| VST3 | 安装双声道输入/输出的效果器，架构与 host worker 匹配。只有 AU 版本不够。 |

macOS 是当前首要运行目标。移动端样式适配不意味着手机可以运行桌面 VST3；Windows/Linux 的真实插件兼容性需要分别验证。

## 1. 安装仓库依赖

```sh
git clone https://github.com/backrunner/wvst.git
cd wvst
npm ci
```

后续命令都在仓库根目录执行。`@wvst/web` 是本地 workspace 包，不需要另行从 npm 安装；也不需要在旁边检出 Svedocs。

## 2. 构建并启动本地 Bridge

先构建两个可执行文件：

```sh
cargo build -p wvst-bridge-server -p wvst-host-worker
```

在终端 A 中启动：

```sh
WVST_TOKEN=local-dev-token \
WVST_HOST_WORKER=target/debug/wvst-host-worker \
  target/debug/wvst-bridge-server serve
```

成功时显示：

```text
wvst-bridge-server listening on ws://127.0.0.1:35876
```

保持终端 A 运行。独立 CLI 要求非空 `WVST_TOKEN`，这里的值只是本地开发示例；不要把真实 token 提交到前端代码或仓库。`WVST_HOST_WORKER` 指向刚构建的 worker，单独构建 Bridge 不会自动产生这个文件。

需要优化构建时，使用匹配的一对 release 文件：

```sh
cargo build --release -p wvst-bridge-server -p wvst-host-worker
WVST_TOKEN=local-dev-token \
WVST_HOST_WORKER=target/release/wvst-host-worker \
  target/release/wvst-bridge-server serve
```

已公开的便携预览包在 [Releases](https://github.com/backrunner/wvst/releases) 获取，按[版本与发布](/docs/zh/releases)选择并校验。草稿尚不能公开下载；上面的源码构建方式继续可用。

## 3. 启动文档与 Studio

终端 B：

```sh
npm run docs:dev
```

这会先构建 Web SDK，再启动 Svedocs。打开终端输出的地址（通常为 `http://localhost:5173`），进入 [Live Studio](/zh/demo)。如果端口被占用，以终端实际输出为准。

开发服务器已提供：

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

在页面控制台运行以下代码，三个值都应该为 `true`：

```js
({
  secureContext: window.isSecureContext,
  crossOriginIsolated: window.crossOriginIsolated,
  sharedArrayBuffer: typeof SharedArrayBuffer === 'function'
})
```

localhost 的安全上下文不自动带来跨源隔离。不要双击 HTML 文件或换成不发送这些响应头的静态服务器。正式托管见[配置与部署](/docs/zh/configuration)。

## 4. 听到第一个效果器

1. 在连接卡片展开高级设置，地址填写 `ws://127.0.0.1:35876`，token 填写 `local-dev-token`，点击连接。首次自动连接没有 token，出现授权提示时在这里补上即可。
2. 选择内置合成器循环，或拖入浏览器支持的音频文件。可以先播放原音确认输出设备正常。
3. 从插件列表选择 VST3 效果器并挂载。列表为空时重新扫描，检查扫描失败提示。
4. 点击播放，调节参数并切换旁路比较声音。在信号路径中确认效果器已启用。
5. 展开处理详情，观察队列及错误计数；输出电平随真实音频变化。

macOS 默认路径包括 `/Library/Audio/Plug-Ins/VST3` 和 `~/Library/Audio/Plug-Ins/VST3`。扫描结果只表示发现了插件，不保证其声道配置、授权或处理功能兼容。

## 5. 结束与重连

暂停播放后移除效果器或断开 Bridge，Studio 会释放所挂载的实例与音频流。重新连接后需要重新挂载效果器。最后在终端 A 按 Ctrl+C 停止服务，在终端 B 按 Ctrl+C 停止文档服务器。

网页刷新不会保存机架和插件状态。自己的应用需要使用 SDK 状态快照接口实现持久化。

## 有问题时从哪里看

| 现象 | 下一步 |
| --- | --- |
| Bridge 无法启动 | 检查 token、端口占用和可执行文件路径。 |
| 网页无法连接 | 确认同一电脑、相同 endpoint，以及浏览器是否拦截本地网络访问。 |
| 可以扫描但挂载失败 | 检查 worker 路径、插件架构、class ID 和声道配置。 |
| 挂载后没有声音 | 先旁路确认原音，再检查实例状态、音频 Worker 授权和错误计数。 |

查看本机诊断：

```sh
WVST_HOST_WORKER=target/debug/wvst-host-worker \
  target/debug/wvst-bridge-server diagnose
```

`diagnose` 检查配置与 worker 发现，不执行真实插件音频处理，也可以在未设置 token 时运行。详细恢复步骤见[故障排查](/docs/zh/troubleshooting)。

## 接下来

- [Demo 指南](/docs/zh/demo-guide)：播放、机架、参数和指标。
- [Web 接入](/docs/zh/web-integration)：从控制连接到完整音频路径及资源释放。
- [配置与部署](/docs/zh/configuration)：token、origin、响应头及环境变量。
- [开发与验证](/docs/zh/development)：检查命令、浏览器回归和真实插件证据。
