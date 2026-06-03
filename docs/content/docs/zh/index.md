---
title: 概览
description: WVST 通过隔离 bridge，把本地 VST3 效果器变成 WebAudio 处理节点。
order: 1
---

# 概览

WVST 是一个 Rust-first 的实验性桥接项目，用来连接 WebAudio 和本地 VST3 插件。Web 应用通过 localhost Bridge Server 扫描插件、创建隔离实例，并用 AudioWorklet + SharedArrayBuffer 路由音频。

浏览器实时音频回调不会阻塞等待本地原生处理。音频会经过有界缓冲、DedicatedWorker transport，以及持有第三方 VST 实例的 host worker 进程。

## 当前形态

- Web SDK：`WVSTClient`、插件扫描/列表 API、实例生命周期 API、loopback AudioWorklet helper 和 bridge worker audio pump。
- Bridge Server：localhost WebSocket 控制面与二进制音频路由、origin/token 校验、metrics、stream 生命周期和 worker supervision。
- Host Worker：隔离进程，用于插件探测、实例生命周期和首版 VST3 processing path。

## 继续阅读

- [快速开始](/docs/zh/getting-started)
- [架构](/docs/zh/architecture)
- [Live Demo](/docs/zh/live-demo)
- [故障排查](/docs/zh/troubleshooting)
