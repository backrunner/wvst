---
title: 快速开始
description: 准备本地 bridge、浏览器 headers 和 WVST Web SDK。
order: 2
---

# 快速开始

WVST 需要浏览器应用和本地 bridge 同时存在。浏览器侧必须运行在安全且 cross-origin isolated 的上下文里，因为低延迟路径依赖 `SharedArrayBuffer`。

## 构建 Web SDK

```sh
npm run build:web
```

文档站会从 workspace 导入 `@wvst/web`，所以运行 demo 前需要先构建 SDK。

## 启动 bridge

```sh
cargo run -p wvst-bridge-server
```

默认 endpoint 是 `ws://127.0.0.1:35876`。Bridge 只监听 loopback，并默认允许 loopback 浏览器 origin。

## 启动文档站

```sh
npm run docs:dev
```

文档 dev server 会发送 `Cross-Origin-Opener-Policy` 和 `Cross-Origin-Embedder-Policy`，让 WVST 低延迟模式可以分配 `SharedArrayBuffer`。

## 使用 demo

打开 [Live Demo](/docs/zh/live-demo)，连接 bridge，选择本地音频文件，扫描插件，然后把一个或多个 2-in/2-out effect VST 挂到 rack 上。
