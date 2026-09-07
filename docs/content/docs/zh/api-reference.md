---
title: API 参考
description: Web SDK 连接选项、实例与参数、状态快照、音频 Worker、MIDI、指标来源和错误处理。
order: 6
---

# API 参考

公共类型由 `@wvst/web` 导出，完整声明随构建生成在 `packages/wvst-web/dist/types`。这里按任务说明主要接口；可直接改用的完整音频 helper 见 [Web 接入](/docs/zh/web-integration)。下方片段共享已连接的 `client`，实例相关示例假设你已创建并选择 `instance` / `instanceId`。

## 连接与握手

```ts
import { WVSTClient, WVSTBridgeError } from '@wvst/web';

const client = await WVSTClient.connect({
  endpoint: 'ws://127.0.0.1:35876',
  clientName: 'my-wvst-app',
  clientVersion: '0.1.0',
  token: 'local-dev-token',
  requireLowLatency: true
});
```

| `ConnectOptions` 字段 | 默认值 | 作用 |
| --- | --- | --- |
| `endpoint` | `ws://127.0.0.1:35876` | 本地 WebSocket 地址。 |
| `clientName` | `@wvst/web` | 诊断中的客户端标识。 |
| `clientVersion` | `0.1.0` | 应用版本，不代替协议版本协商。 |
| `token` | 未设置 | 独立 Bridge CLI 需要与 `WVST_TOKEN` 一致的值。 |
| `requireLowLatency` | `true` | 连接前检查 SAB 与跨源隔离。关闭只跳过检查，不提供音频 fallback。 |

`WVSTClient.lowLatencyPrerequisites()` 返回 `sharedArrayBuffer` 和 `crossOriginIsolated` 两个布尔值；应用还应检查 `isSecureContext` 和 AudioWorklet。`client.hello` 保存握手协商的协议/音频帧版本、Bridge 身份和能力要求。普通 RPC response 不保证重复包含版本字段。

`client.close()` 关闭控制连接，不代替实例销毁、音频节点断开或 Worker 终止。

## 插件发现

| 方法 | 返回或用途 |
| --- | --- |
| `client.plugins.list({ rescan, paths })` | 返回 `PluginScanReport`，按需重新扫描。 |
| `client.plugins.scan({ paths })` | 扫描默认路径或显式路径。 |
| `client.plugins.factoryInfo({ path })` | 在 host worker 中查询 factory 和 class 信息。 |

`PluginScanReport` 有 `plugins` 与 `failures` 数组；扫描可以部分成功。插件 descriptor 包含 `pluginId`、`path`、`format`、名称、可选厂商/版本、`metadataSource` 和 `classes`。class ID 可能在静态元数据中缺失，需要 factory 查询。先处理空数组与失败项，再让用户选择。

## 实例与音频流生命周期

| SDK 方法 | 行为 |
| --- | --- |
| `instances.create(options)` | 创建实例并返回 `InstanceDescriptor`，包含独立 `instanceId` 与 `streamId`。 |
| `instances.start({ instanceId })` | 启动插件处理。 |
| `instances.status({ instanceId })` | 查询当前实例/worker 状态。 |
| `instances.stop({ instanceId })` | 停止处理，保留实例。 |
| `instances.restart({ instanceId })` | 请求 worker 恢复；应用需要重新确认状态与图连接。 |
| `instances.openStream({ instanceId })` | 打开已关闭的流。 |
| `instances.closeStream({ instanceId })` | 关闭对应流。 |
| `instances.destroy({ instanceId })` | 释放原生实例。 |

`create` 的核心选项是 `pluginId`、`classId`、`sampleRate`、`maxBlockFrames`、`inputChannels`、`outputChannels`；还可按类型定义选择 bus。使用 AudioContext 的实际采样率。新实例通常已有打开的流，但处理仍需要 `start()`。

控制接口使用 `instanceId`，音频 Worker 的 stream 命令使用 `streamId`。创建时返回的 descriptor 是快照，不会随状态变化自动更新。运行中重新读取 `status()` 或 `runtimeSnapshot()`。

## 参数编辑

```ts
const { parameters } = await client.instances.parameters({ instanceId });
const editable = parameters.find((p) => !p.flags.readOnly && !p.flags.hidden);
if (editable) {
  await client.instances.parameterEdit({
    instanceId,
    parameterId: editable.id,
    valueNormalized: 0.72
  });
  const display = await client.instances.parameterInfo({
    instanceId,
    parameterId: editable.id
  });
  console.log(display.valueNormalized, display.valuePlain, display.valueString);
}
```

值域为 normalized `0..1`；不要把 Hz、dB 等显示值直接写入 `valueNormalized`。`parameterInfo()` 提供插件支持的 plain/display 转换；离散参数根据 `stepCount` 处理，隐藏与只读参数根据 flags 过滤。

`parameterEdit()` 聚合 begin/perform/end gesture，适合一次 UI 修改。拖动时可分别调用 `parameterBeginEdit()`、`parameterPerformEdit()`、`parameterEndEdit()`；sample-offset 自动化则通过音频 Worker 的 `sendParameterEvents()` 排入音频块，而不是从 UI 定时器伪造采样精度。

## 保存与恢复状态

```ts
import {
  createWVSTInstanceStateSnapshot,
  instanceStateSnapshotToSetStateOptions
} from '@wvst/web';

const state = await client.instances.getState({ instanceId: instance.instanceId });
const snapshot = createWVSTInstanceStateSnapshot(state, instance);
const serialized = JSON.stringify(snapshot);

// Restore into the selected compatible target descriptor.
const options = instanceStateSnapshotToSetStateOptions(
  target.instanceId, snapshot, target
);
await client.instances.setStateAndRefresh(options);
```

`instance` 与 `target` 都是已创建实例的 descriptor。保存的 JSON 由应用持久化；从文件读取后先验证 schema 和字段，不要仅靠 TypeScript 类型断言信任外部数据。helper 会检查支持的 schema、可恢复内容和提供的目标配置兼容性。

component/controller state 是插件私有的 base64 数据。快照检查不能保证不同插件版本完全兼容；恢复后用返回的 metadata 更新参数 UI。插件不提供可保存状态时，不应伪造空状态成功。

## AudioWorklet 与传输 Worker

| 接口 | 职责 |
| --- | --- |
| `createLoopbackSharedBuffers(options)` | 分配输入/输出 SAB 与计数器，默认容量四个 quantum。 |
| `configureLoopbackAudioWorkletNode(node, buffers)` | 把共享缓冲配置交给已创建节点。 |
| `createLoopbackAudioWorkletNode(context, options)` | 加载 processor 并创建节点；多节点场景应统一管理模块加载。 |
| `WVSTBridgeWorkerClient.connect(endpoint)` | 打开 Worker socket；随后还需 `request('bridge.hello', params)` 授权。 |
| `startAudioStream(options)` | 使用匹配的 stream ID、采样率、块大小、声道与缓冲启动传输。 |
| `stopAudioStream(streamId)` | 停止该浏览器音频流。 |
| `close()` | 关闭 Worker 传输；持有原生 Worker 的调用方还需 `terminate()`。 |

同一个 AudioContext 只注册一次 `wvst-loopback` processor。完整双握手及清理顺序见 [Web 接入](/docs/zh/web-integration)。

## MIDI、设备与原生共享内存

`createWVSTVirtualKeyboard()` 构造 note、CC、pitch bend 和 aftertouch 事件；`createWVSTWebMidiAdapter()` 转换 Web MIDI 消息。事件携带 channel 和 `sampleOffset`；偏移必须落在当前音频块范围。传输使用 `sendMidiEvents({ streamId, events })`，应在流启动后发送。Web MIDI 可用性及设备权限由浏览器决定。

`createWVSTAudioDeviceSession()` 管理设备输入与输出路由，音频设备权限仍由浏览器/使用者授予。`createWVSTSharedMemoryPumpSession()` 管理 Bridge 侧文件映射共享内存处理；它与浏览器 SharedArrayBuffer 不是同一个内存空间，浏览器不会直接映射本地文件。

## 指标从哪里读

| 来源 | 能说明什么 |
| --- | --- |
| `readLoopbackMetrics(buffers)` | 输入输出帧、pending quanta、underflow/overflow、丢弃/迟到事件、传输失败。 |
| `client.metrics()` | Bridge 路由、序号异常、抖动与路由延迟直方图，以及 shared-memory pump 指标。 |
| `instances.status()` / `runtimeSnapshot()` | 插件报告的 `latencySamples`、运行能力与 worker/数据面状态。 |
| `client.events()` / `onEvent()` | worker 失败、恢复、隔离及 metadata 失效等生命周期事件。 |
| 外部 loopback 测量与 testkit | 使用观察样本计算端到端往返分位数及稳定性预算。 |

`latencySamples / sampleRate * 1000` 只转换插件声明的延迟，不包含浏览器或传输。`capacityQuanta` 也不是固定延迟。CPU 与音频健康结论需要真实测量，不能从队列长度推算。

## 错误与订阅

```ts
const unsubscribe = client.onEvent((event) => {
  console.log(event.kind.type, event.kind);
});

try {
  await client.instances.status({ instanceId });
} catch (error) {
  if (error instanceof WVSTBridgeError) {
    console.error(error.code, error.message, error.data);
  } else {
    console.error(error);
  }
}

unsubscribe();
client.close();
```

控制 client 的 RPC 失败使用 `WVSTBridgeError`，包含 `code`、`message` 和可选 `data`；连接/前置条件错误使用普通 `Error`。Worker 包装层会把失败作为错误消息返回，不要假设它也保留 `WVSTBridgeError` 的结构化字段。

`onEvent()` 返回取消订阅函数。异步刷新 metadata 时自行捕获错误，或使用 `onMetadataInvalidated(listener, { onError })`。收到需要重建音频图或重载 component 的刷新策略后，应用必须处理对应生命周期，读取 metadata 本身不会自动重建图。

## 常用控制面映射

| Web SDK | JSON-RPC |
| --- | --- |
| `client.metrics()` / `events()` | `bridge.metrics` / `bridge.events` |
| `plugins.list()` / `scan()` / `factoryInfo()` | `plugin.list` / `plugin.scan` / `plugin.factoryInfo` |
| `instances.create()` / `start()` / `destroy()` | `instance.create` / `instance.start` / `instance.destroy` |
| `instances.parameterEdit()` | `instance.parameter.edit` |
| `instances.setStateAndRefresh()` | `instance.state.setAndRefresh` |
| `instances.runtimeSnapshot()` | `instance.runtime.snapshot` |
| `instances.openStream()` / `closeStream()` | `stream.open` / `stream.close` |
| `instances.sharedMemoryPumpStart()` | `stream.sharedMemory.pump.start` |

高级 units、program data、connection notification 和事件 payload 见导出的类型以及[架构](/docs/zh/architecture)。
