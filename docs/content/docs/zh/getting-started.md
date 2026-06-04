---
title: 快速开始
description: 构建 Web SDK、启动本地 Bridge、提供 cross-origin isolated 文档站，并创建真实 WVST 音频链路。
order: 2
---

# 快速开始

WVST 有两个部分：

- 使用 `@wvst/web` 的浏览器应用。
- 本机 `wvst-bridge-server` 进程，负责插件发现、worker supervision 和音频路由。

低延迟路径要求浏览器页面处于 secure context 且 cross-origin isolated。localhost 属于 secure context，但服务器仍需要发送 COOP/COEP headers 才能使用 `SharedArrayBuffer`。

## 安装 workspace 依赖

在仓库根目录运行：

```sh
npm install
```

文档 app 属于 npm workspace，并从 `packages/wvst-web` 导入 `@wvst/web`。

## 构建 Web SDK

```sh
npm run build:web
```

这会运行 `@wvst/web` 的 TypeScript build 和 Rollup bundle。包导出浏览器入口、worker 入口和 AudioWorklet 入口：

```ts
import { WVSTClient } from "@wvst/web";
import BridgeWorker from "@wvst/web/bridge-worker?worker";
import loopbackProcessorUrl from "@wvst/web/loopback-processor?url";
```

请使用当前 bundler 支持的 worker 和 URL import 语法。文档站使用 Vite，因此支持 `?worker` 和 `?url`。

## 启动 Bridge

```sh
cargo run -p wvst-bridge-server
```

默认 endpoint：

```txt
ws://127.0.0.1:35876
```

常用命令：

```sh
cargo run -p wvst-bridge-server -- serve
cargo run -p wvst-bridge-server -- diagnose
```

`diagnose` 会输出 config、platform、env 和 host worker discovery 的 JSON。

## Bridge 环境变量

测试授权、限制和 worker 行为时常用：

| 变量 | 用途 |
| --- | --- |
| `WVST_BIND_ADDR` | 覆盖默认 `127.0.0.1:35876` bind address。 |
| `WVST_TOKEN` | 要求 `bridge.hello` 提供 token。 |
| `WVST_ALLOWED_ORIGINS` | 逗号分隔的 origin allowlist。 |
| `WVST_ALLOW_LOOPBACK_ORIGINS` | 设为 `0` 或 `false` 后不再自动允许 loopback origins。 |
| `WVST_HOST_WORKER` | 覆盖 host worker executable discovery。 |
| `WVST_WORKER_AUTO_RESTART` | 设为 `0` 或 `false` 后关闭 worker 自动重启。 |
| `WVST_MAX_WORKER_INSTANCES` | 限制并发 worker instances。默认 `64`。 |
| `WVST_WORKER_QUARANTINE_FAILURES` | 插件进入 quarantine 前允许的失败次数。默认 `3`。 |
| `WVST_MAX_CONTROL_MESSAGE_BYTES` | 限制 JSON 控制消息大小。 |
| `WVST_WORKER_MEMORY_LIMIT_BYTES` | 支持的平台上限制 supervised worker address space。 |
| `WVST_WORKER_CPU_TIME_LIMIT_SECONDS` | 支持的平台上限制 supervised worker CPU time。 |
| `WVST_WORKER_LINUX_CGROUP_PARENT` | Linux cgroup parent。 |
| `WVST_WORKER_LINUX_CGROUP_MEMORY_MAX_BYTES` | Linux cgroup memory max。 |
| `WVST_WORKER_LINUX_CGROUP_CPU_QUOTA_MICROS` | Linux cgroup CPU quota。 |
| `WVST_WORKER_LINUX_CGROUP_CPU_PERIOD_MICROS` | Linux cgroup CPU period。 |

示例：

```sh
WVST_TOKEN=dev-token \
WVST_ALLOWED_ORIGINS=http://127.0.0.1:5173 \
cargo run -p wvst-bridge-server
```

## 启动文档站

```sh
npm run docs:dev
```

文档 dev server 会先构建 `@wvst/web`，再启动 Svedocs。项目 Vite 配置会发送：

```txt
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

SvelteKit hooks 也会加同样 headers，因此 production-style 本地构建也满足低延迟前置条件。

## 最小 Web SDK 流程

真实插件实例的最小控制面路径：

```ts
const prerequisites = WVSTClient.lowLatencyPrerequisites();
if (!prerequisites.sharedArrayBuffer || !prerequisites.crossOriginIsolated) {
  throw new Error("WVST requires SharedArrayBuffer and cross-origin isolation");
}

const client = await WVSTClient.connect({
  endpoint: "ws://127.0.0.1:35876",
  clientName: "my-wvst-app",
  clientVersion: "0.1.0",
  requireLowLatency: true,
  token: "dev-token"
});

const report = await client.plugins.list({ rescan: true });
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
```

`client.hello` 包含协议协商结果和 Bridge metrics。连接后可以调用 `client.metrics()` 和 `client.events()`。

## 接入 AudioWorklet 路径

浏览器 live path 使用 AudioWorklet node 加 DedicatedWorker audio pump：

```ts
const worker = new BridgeWorker();
const bridgeWorker = new WVSTBridgeWorkerClient({ worker });
await bridgeWorker.connect("ws://127.0.0.1:35876");

const buffers = createLoopbackSharedBuffers({
  frames: 128,
  inputChannels: 2,
  outputChannels: 2,
  capacityQuanta: 4
});

await audioContext.audioWorklet.addModule(loopbackProcessorUrl);
const node = new AudioWorkletNode(audioContext, "wvst-loopback", {
  numberOfInputs: 1,
  numberOfOutputs: 1,
  outputChannelCount: [2],
  channelCount: 2,
  channelCountMode: "explicit"
});

configureLoopbackAudioWorkletNode(node, buffers);
source.connect(node).connect(audioContext.destination);

await bridgeWorker.startAudioStream({
  streamId: instance.streamId,
  sampleRate: instance.sampleRate,
  frames: 128,
  inputChannels: 2,
  outputChannels: 2,
  buffers
});
```

使用 `readLoopbackMetrics(buffers)` 读取 pending input/output quanta、underflow、overflow、dropped events、late events 和 transport failures。

## 参数与 State 基础

实例 ready 后：

```ts
const parameters = await client.instances.parameters({
  instanceId: instance.instanceId
});

const cutoff = parameters.parameters.find((parameter) =>
  parameter.title?.toLowerCase().includes("cutoff")
);

if (cutoff) {
  await client.instances.parameterEdit({
    instanceId: instance.instanceId,
    parameterId: cutoff.id,
    valueNormalized: 0.72
  });
}

const state = await client.instances.getState({ instanceId: instance.instanceId });
const snapshot = createWVSTInstanceStateSnapshot(state, instance);
```

使用 `instanceStateSnapshotToSetStateOptions()` 把 snapshot 恢复到兼容实例。

## MIDI 基础

Instrument 插件和支持 MIDI 的 effect 可以通过 bridge worker 接收事件：

```ts
const keyboard = createWVSTVirtualKeyboard({
  buffers,
  sendMidiEvents: (events) =>
    bridgeWorker.sendMidiEvents({
      streamId: instance.streamId,
      events
    })
});

await keyboard.noteOn(60, 0.9);
await keyboard.noteOff(60);
```

helper 会校验 channel、velocity、pitch bend 和 sample offset。Web MIDI 可以用 `createWVSTWebMidiAdapter()` 转换。

## 运行检查

```sh
npm run docs:check
npm run docs:build -- --no-og
```

整个仓库：

```sh
npm run check
```

`npm run check` 会运行 Rust tests 和 Web SDK type check。
