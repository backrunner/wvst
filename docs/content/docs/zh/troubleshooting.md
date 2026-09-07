---
title: 故障排查
description: 诊断浏览器隔离、Bridge 授权、插件发现、worker lifecycle、shared-memory transport 和实时 metrics。
order: 8
---

# 故障排查

先定位故障发生在哪一层，再修改设置。以下 SDK 片段假设已导入对应 helper，且 `client`、`instanceId`、`buffers` 等来自当前应用会话。

| 现象 | 优先检查 |
| --- | --- |
| 启动报缺少 token | 独立 CLI 必须设置非空 `WVST_TOKEN`。 |
| socket 连接不到 | Bridge 是否运行、端口是否匹配、浏览器是否拦截本地访问。 |
| 可以扫描，但音频持续失败 | 音频 Worker 是否独立完成带 token 的握手。 |
| 找得到插件，却挂载不了 | worker 可执行文件、插件 CPU 架构、class ID、VST3 总线。 |
| 原音正常，启用效果器后静音 | 实例状态、流配置、音频错误计数和插件能力。 |
| 本地正常，部署后失效 | 最终页面 COOP/COEP、资源 URL、实际网页 origin。 |

回到[快速开始](/docs/zh/getting-started)的双终端配置，能排除不少环境差异。

## SharedArrayBuffer 不可用

在浏览器中检查：

```ts
WVSTClient.lowLatencyPrerequisites();
```

两个值都必须为 true：

- `sharedArrayBuffer`
- `crossOriginIsolated`

应用服务器需要发送：

```txt
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

本地文档站请使用 `npm run docs:dev`。页面在 localhost 上并不自动代表 COOP/COEP 已满足。

## Bridge 连接失败

启动 Bridge：

```sh
cargo build -p wvst-bridge-server -p wvst-host-worker
WVST_TOKEN=local-dev-token \
WVST_HOST_WORKER=target/debug/wvst-host-worker \
  target/debug/wvst-bridge-server serve
```

默认 endpoint：

```txt
ws://127.0.0.1:35876
```

如果 endpoint 不同，请先设置 `WVST_BIND_ADDR` 再启动 Bridge，并在应用中填入对应 WebSocket URL。

运行 diagnostics：

```sh
cargo run -p wvst-bridge-server -- diagnose
```

## Session 未授权

独立 CLI 启动时要求非空 `WVST_TOKEN`。除 `bridge.hello` 外，控制方法和二进制音频都需要已授权的连接；把同一个 token 传给 `WVSTClient.connect({ token })`。音频 Worker 是另一个 socket，必须再次执行 `bridge.hello`。

如果 origin 检查失败：

- 把精确 origin 加入 `WVST_ALLOWED_ORIGINS`。
- 本地开发保持 `WVST_ALLOW_LOOPBACK_ORIGINS=true`。
- 不要用 wildcard origin 授予强插件权限。

## 没有插件

执行 rescan：

```ts
await client.plugins.scan();
```

macOS 下 scanner 使用标准 VST3 路径，例如：

- `/Library/Audio/Plug-Ins/VST3`
- `~/Library/Audio/Plug-Ins/VST3`

也可以传入显式路径：

```ts
await client.plugins.scan({
  paths: ["/Library/Audio/Plug-Ins/VST3"]
});
```

需要更深的 metadata 时：

```ts
await client.plugins.factoryInfo({ path: "/path/to/Plugin.vst3" });
```

Factory info 通过 host worker 查询，不会在 Bridge Server 进程中加载 VST3 代码。

## Instance 创建失败

确认 `pluginId` 和可选 `classId` 来自最新 scan report。使用 `AudioContext` 的实际 `sampleRate`，并选择合理 block size：

```ts
await client.instances.create({
  pluginId,
  classId,
  sampleRate: Math.round(audioContext.sampleRate),
  maxBlockFrames: 128,
  inputChannels: 2,
  outputChannels: 2
});
```

不支持的 bus arrangement 通常会表现为 worker error 或缺失 runtime capabilities。检查：

```ts
await client.instances.status({ instanceId });
await client.instances.runtimeSnapshot({ instanceId, includeRecentEvents: true });
```

## 挂载后没有声音

按层排查：

1. Browser media element 正在播放。
2. WebAudio source 已连接到 active slot chain。
3. instance 处于 `processing` state。
4. stream 是 `open`。
5. Bridge worker audio stream 用匹配的 `streamId`、`sampleRate`、frames 和 channel counts 启动。
6. `readLoopbackMetrics(buffers)` 没有持续增长 `underflows` 或 `transportFailures`。
7. `client.metrics()` 没有 route failures 或 unmatched streams。

当前 live rack 针对 2-in/2-out effect 优化。Instrument 可以通过 MIDI APIs 驱动，但不是第一版 live demo workflow。

## AudioWorklet Processor 注册失败

浏览器不允许在同一个 `AudioContext` 中重复注册同名 processor。请只加载一次 `@wvst/web/loopback-processor`，并复用 promise：

```ts
workletModule ??= audioContext.audioWorklet.addModule(loopbackProcessorUrl);
await workletModule;
```

之后每个 rack slot 创建新的 `AudioWorkletNode`。

## Metrics 显示 Underflow 或 Overflow

Loopback metrics 是浏览器侧 counters：

- `underflows`：worklet 需要输出时，处理后的 output 尚未准备好。
- `overflows`：input 或 event queues 超出容量。
- `droppedInputQuanta` / `droppedOutputQuanta`：ring pressure。
- `droppedMidiEvents` / `droppedParameterEvents`：event queue pressure。
- `lateMidiEvents` / `lateParameterEvents`：event 目标 sequence 已经过期。
- `transportFailures`：DedicatedWorker 或 Bridge transport 失败。

Bridge metrics 提供 server-side 上下文：

```ts
const metrics = await client.metrics();
```

重点看 route failures、invalid frame headers/lengths、unmatched streams、sequence gaps、duplicate/out-of-order/late frames、backpressure drops、shared-memory pump errors 和 latency histograms。

## Worker 失败或进入 Quarantine

监听 Bridge events：

```ts
const unsubscribe = client.onEvent((event) => {
  console.log(event.kind);
});
```

重要 worker events：

- `worker-failed`
- `worker-recovering`
- `worker-recovered`
- `worker-recovery-failed`
- `worker-quarantined`
- `worker-quarantine-released`
- `worker-policy-decision`

自动重启由 `WVST_WORKER_AUTO_RESTART` 控制。连续失败阈值由 `WVST_WORKER_QUARANTINE_FAILURES` 控制。

如果 worker 根本无法启动，运行 `cargo run -p wvst-bridge-server -- diagnose`，检查 `WVST_HOST_WORKER`、可执行权限、CPU architecture 和本地 package layout。

## 运行时 Metadata 改变

有些 VST3 插件会发出 component handler restart 或 metadata invalidation events。使用：

```ts
const unsubscribeMetadata = client.onMetadataInvalidated(
  (result) => { console.log(result.refreshed); },
  {
    includeParameterValues: true,
    onError: (_event, error) => { console.error(error); }
  }
);
// On teardown:
unsubscribeMetadata();
```

如果 refresh policy 是 `rebuild-audio-graph` 或 `reload-component`，应用应 rebuild 受影响 graph 或重新创建 instance。

## State Restore 失败

WVST 把 VST3 component/controller state 当作 opaque base64。不要解析或编辑插件私有 state。

恢复前先检查：

```ts
const compatibility = checkWVSTInstanceStateSnapshotCompatibility(
  snapshot,
  descriptor
);
```

兼容性检查会比较 snapshot 中存在的 plugin id、class id、sample rate、block size 和 channel counts。

## MIDI 没有触发 Instrument

检查：

- instance 使用 `inputChannels: 0`，或使用 instrument 支持的 arrangement。
- 如果依赖 CC/pitch bend mapping，worker runtime capabilities 中包含 `midiMapping`。
- `sampleOffset` 小于 block frame count。
- 调用 `sendMidiEvents` 前 Bridge worker stream 已启动。

键盘测试：

```ts
const keyboard = createWVSTVirtualKeyboard(target);
await keyboard.noteOn(60, 0.9);
await keyboard.noteOff(60);
```

## Shared-Memory Pump 不运行

检查 data-plane snapshot：

```ts
await client.instances.runtimeSnapshot({
  instanceId,
  includeRecentEvents: true
});
```

snapshot 包含：

- `dataPlane.sharedMemory`
- `dataPlane.sharedMemoryPump`

常见原因包括缺少 shared-memory descriptor、pump 已停止、input underrun、output backpressure、worker error、event queue overflow，或平台 mmap backend 不可用。

## Stability 和 Package Evidence

验证某台机器或安装包时可使用 testkit：

```sh
cargo run -p wvst-testkit --bin wvst-stability-budget -- \
  --snapshot latency-snapshot.json \
  --budget budget.json \
  --webaudio webaudio-loopback-metrics.json \
  --bridge bridge-metrics.json
```

```sh
cargo run -p wvst-testkit --bin wvst-package-evidence -- \
  --manifest wvst-package-manifest.json \
  --verify-report wvst-verify-report.json \
  --budget package-budget.json
```

`.agents/*.example.json` 提供了这些报告的起始 fixtures。


## 控制连接成功但音频未授权

`WVSTBridgeWorkerClient.connect()` 只打开 WebSocket。随后在传输 Worker 上执行：

```ts
await bridgeWorker.request('bridge.hello', {
  ...client.createHelloRequest().params,
  token: tokenFromUser
});
```

`createHelloRequest()` 不保存原连接 token，需显式补上。完整流程见 [Web 接入](/docs/zh/web-integration)。

## 文件、播放与重连问题

浏览器拒绝自动播放时，在用户点击播放的处理函数内调用 `audioContext.resume()`，再调用媒体元素的 `play()`。先用内置循环或 WAV 排除格式兼容问题。大于 50 MB 的文件不生成波形，是 Studio 的内存控制行为。

同一个媒体元素不能重复创建 `MediaElementAudioSourceNode`。重连时复用已有 source；只在元素与整个图一起销毁时释放它。旧的 WebSocket 关闭后需要新建控制/Worker 连接、重新握手并挂载新实例。

## 部署后资源或本地网络访问失败

如果 Worker 或 worklet 请求返回 `text/html`，检查 SPA fallback 和静态资源路径。页面必须在最终响应中获得 COOP/COEP，静态站点不会运行 SvelteKit server hook。

若隔离状态正常而 WebSocket 被拦截，查看浏览器控制台的混合内容、本地网络权限和 origin 错误。不要把 `ws://` 简单改成 `wss://`；Bridge CLI 没有自动启用 TLS。详见[配置与部署](/docs/zh/configuration)。

## 保存一次有用的诊断

```ts
const diagnostics = {
  hello: client.hello,
  metrics: await client.metrics(),
  events: await client.events(),
  runtime: await client.instances.runtimeSnapshot({
    instanceId, includeRecentEvents: true
  })
};
console.log(JSON.stringify(diagnostics, null, 2));
```

保留故障前后的计数增量、复现步骤、浏览器/系统版本和插件版本。分享前移除 token 和不必要的个人路径。真实插件和长期稳定性验证方法见[开发与验证](/docs/zh/development)。
