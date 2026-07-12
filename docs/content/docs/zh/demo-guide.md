---
title: Demo Guide
description: 使用本地 bridge 和真实 VST3 效果器运行 WVST 浏览器 rack。
order: 4
---

# Demo Guide

独立的 [Demo](/zh/demo) 是一条真实的 browser-to-native 音频链路。它加载本地音频文件，连接本地 Bridge Server，发现 VST3 metadata，并通过隔离 worker instance 路由音频。

## 浏览器前置条件

- 使用 localhost 或 HTTPS 提供站点。
- 发送 `Cross-Origin-Opener-Policy: same-origin`。
- 发送 `Cross-Origin-Embedder-Policy: require-corp`。
- 确认 `crossOriginIsolated` 和 `SharedArrayBuffer` 可用。

缺少前置条件时，demo 会显示明确错误，不会退化为伪造的播放效果。

## 启动 Bridge

#### 下载 release

从 [WVST GitHub Releases](https://github.com/backrunner/wvst/releases) 下载对应平台的 package。里面包含 Bridge Server、隔离的 host worker 和 service installer。

仓库目前还没有发布二进制。现在可以在仓库根目录构建两个 binary：

```sh
cargo build --release -p wvst-bridge-server -p wvst-host-worker
WVST_HOST_WORKER=target/release/wvst-host-worker \
  target/release/wvst-bridge-server serve
```

保持进程或已安装的 service 运行，然后打开 [Demo](/zh/demo)。页面加载时会自动尝试一次默认 endpoint `ws://127.0.0.1:35876`。修改地址或 Token 后，可以使用连接按钮重试。

首次连接会协商 Bridge 版本、origin policy、可选 token 和插件能力。

## Rack 生命周期

每个 slot 拥有一个 WVST instance、一份 shared-buffer、一个 `AudioWorkletNode` 和一条 Bridge worker stream。浏览器实时线程不等待 native processing，worker 负责 transport。

移除 slot 时，rack 会依次断开 worklet、停止 stream、停止 processing、关闭 stream、销毁 instance，并重建剩余 graph。

## 观察指标

rack 会展示 pending quanta、underflow、overflow、dropped events、worker restart 和 transport failure。通过这些 counters 可以区分插件故障、浏览器前置条件和 bridge 配置问题。

公共方法与控制面接口请参考 [API Reference](/docs/zh/api-reference)。
