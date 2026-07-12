---
title: API Reference
description: Web SDK 公共方法、实时音频 helper、Bridge 操作和 runtime error contract。
order: 5
---

# API Reference

这份 reference 描述示例和独立 Demo 当前使用的公共接口。类型由 `@wvst/web` 导出，控制面方法通过协商后的 transport 发送到本地 Bridge Server。

## 连接

```ts
import { WVSTClient } from '@wvst/web';

const client = await WVSTClient.connect({
  endpoint: 'ws://127.0.0.1:35876',
  clientName: 'my-wvst-app',
  clientVersion: '0.1.0',
  requireLowLatency: true
});
```

`connect()` 会检查 secure context、cross-origin isolation、token policy 和 Bridge protocol version。低延迟前置条件不满足时，会在打开 transport 前抛出错误，不会静默切换模式。

## 插件

```ts
const report = await client.plugins.list({ rescan: false });
await client.plugins.scan();
const factory = await client.plugins.factoryInfo({
  path: report.plugins[0].path
});
```

`PluginScanReport` 包含已发现的 plugin descriptors 和 scan failures。Descriptor 提供 plugin id、format、path、metadata source、vendor、version 和 classes。Runtime buses、parameters、units、programs、latency 与 tail 会在创建 instance 后提供。

## Instance

```ts
const choice = report.plugins[0];
const pluginClass = choice.classes[0];

const instance = await client.instances.create({
  pluginId: choice.pluginId,
  classId: pluginClass?.classId,
  sampleRate: audioContext.sampleRate,
  maxBlockFrames: 128,
  inputChannels: 2,
  outputChannels: 2
});

await client.instances.start({ instanceId: instance.instanceId });
await client.instances.stop({ instanceId: instance.instanceId });
await client.instances.restart({ instanceId: instance.instanceId });
await client.instances.destroy({ instanceId: instance.instanceId });
```

默认隔离策略为每个 instance 使用独立 host worker。worker 崩溃会由 Bridge 上报，不会拖垮 Bridge 进程。

## Audio Node

```ts
const buffers = createLoopbackSharedBuffers({
  frames: 128,
  inputChannels: 2,
  outputChannels: 2,
  capacityQuanta: 4
});

const node = await createLoopbackAudioWorkletNode(audioContext, {
  processorUrl: loopbackProcessorUrl,
  buffers
});

source.connect(node).connect(audioContext.destination);
```

AudioWorklet 通过 `WVSTBridgeWorkerClient` 交换有界 buffer。`process()` 不等待 native processing，不做网络 I/O，也不解析 JSON。完整 worker stream 设置请参考[快速开始](/docs/zh/getting-started)。

## 参数与 State

```ts
await client.instances.parameterSet({
  instanceId: instance.instanceId,
  parameterId,
  valueNormalized: 0.72
});

const info = await client.instances.parameterInfo({
  instanceId: instance.instanceId,
  parameterId
});

const state = await client.instances.getState({
  instanceId: instance.instanceId
});
const snapshot = createWVSTInstanceStateSnapshot(state, instance);
```

带 gesture 的自动化使用 `parameterBeginEdit`、`parameterPerformEdit` 和 `parameterEndEdit`，也可以使用聚合方法 `parameterEdit`。持久化 state 通过 `instanceStateSnapshotToSetStateOptions()` 和 `client.instances.setState()` 恢复。插件 state 保持为 opaque base64 data。

## MIDI 与设备

包导出 `createWVSTVirtualKeyboard()`、`createWVSTWebMidiAdapter()` 和 `createWVSTAudioDeviceSession()`。Note、CC、pitch bend、aftertouch 和 raw MIDI event 都携带 channel 与 sample-offset。

## Shared Memory 与 Metrics

浏览器 loopback 路径使用 `createLoopbackSharedBuffers()`、`configureLoopbackAudioWorkletNode()` 和 `readLoopbackMetrics()`。指标包括 configured latency、round-trip、jitter、underflow、overflow、late frame、worker restart、plugin latency 和 process CPU。

## Bridge 操作

控制面使用带版本的 JSON-RPC 消息。常用方法包括：

- `bridge.hello`、`bridge.metrics`、`bridge.events`
- `plugin.scan`、`plugin.list`、`plugin.factoryInfo`
- `instance.create`、`instance.list`、`instance.status`、`instance.start`、`instance.stop`、`instance.restart`、`instance.destroy`
- `instance.parameters`、`instance.parameter.get`、`instance.parameter.info`、`instance.parameter.beginEdit`、`instance.parameter.performEdit`、`instance.parameter.endEdit`、`instance.parameter.edit`
- `instance.getState`、`instance.setState`、`instance.state.setAndRefresh`
- `stream.open`、`stream.close`、`stream.sharedMemory.create`、`stream.sharedMemory.process`、`stream.sharedMemory.pump.start`、`stream.sharedMemory.pump.status`

每个 response 都包含 protocol 或 Bridge version，用于诊断 client 与服务端版本不匹配。

## 错误

Bridge JSON-RPC 失败会 reject `WVSTBridgeError`，其中包含数字 `code`、人类可读的 `message` 和可选结构化 `data`。WebSocket 连接失败或缺少低延迟浏览器前置条件时，会在 RPC session 建立前 reject 普通 `Error`。

Bridge event 会报告 worker exit、recovery、quarantine、stream state、metrics 和 VST3 metadata invalidation。使用 `client.onEvent()` 订阅，并在 teardown 时调用返回的 unsubscribe function。
