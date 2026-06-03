---
title: 故障排查
description: WVST 文档 demo 和低延迟浏览器路径的常见设置问题。
order: 5
---

# 故障排查

## SharedArrayBuffer 不可用

请使用 `npm run docs:dev`，或使用另一个会发送以下 headers 的服务器：

```txt
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

这些 headers 缺失时，WVST 低延迟模式会明确失败。

## Bridge 连接失败

启动 bridge，并保持默认 endpoint，除非你修改过 `WVST_BIND_ADDR`：

```sh
cargo run -p wvst-bridge-server
```

如果你设置了 `WVST_TOKEN`，连接前需要在 demo 中填入同一个 token。

## 没有插件

在 demo 里执行 rescan。macOS 下 scanner 会检查标准 VST3 路径，包括 `/Library/Audio/Plug-Ins/VST3` 和用户 Library 路径。

## 挂载后没有声音

第一版 demo 预期 stereo effect 插件。Instrument 插件、不支持的 bus arrangement、已停止 stream 或崩溃 worker 都会显示 slot error 或 underflow/overflow 计数。
