---
title: Getting Started
description: Build the Web SDK, start the local Bridge, serve cross-origin isolated docs, and create a real WVST audio path.
order: 2
---

# Getting Started

WVST has two halves:

- A browser app using `@wvst/web`.
- A local `wvst-bridge-server` process that owns plugin discovery, worker supervision, and audio routing.

For the low-latency path, the browser page must be secure and cross-origin isolated. Localhost counts as a secure context, but the server still has to send the COOP/COEP headers needed for `SharedArrayBuffer`.

## Install Workspace Dependencies

From the repository root:

```sh
npm install
```

The docs app is part of the npm workspace and imports `@wvst/web` from `packages/wvst-web`.

## Build the Web SDK

```sh
npm run build:web
```

This runs the `@wvst/web` TypeScript build and Rollup bundle. The package exports the browser entry plus worker and AudioWorklet entrypoints:

```ts
import { WVSTClient } from "@wvst/web";
import BridgeWorker from "@wvst/web/bridge-worker?worker";
import loopbackProcessorUrl from "@wvst/web/loopback-processor?url";
```

Use your bundler's worker and URL import syntax. The docs site uses Vite, so the `?worker` and `?url` imports are supported.

## Start the Bridge

```sh
cargo run -p wvst-bridge-server
```

Default endpoint:

```txt
ws://127.0.0.1:35876
```

Useful Bridge commands:

```sh
cargo run -p wvst-bridge-server -- serve
cargo run -p wvst-bridge-server -- diagnose
```

`diagnose` prints JSON for config, platform, env, and host worker discovery.

## Bridge Environment Variables

Use these when testing authorization, limits, and worker behavior:

| Variable | Purpose |
| --- | --- |
| `WVST_BIND_ADDR` | Override the default `127.0.0.1:35876` bind address. |
| `WVST_TOKEN` | Require a token during `bridge.hello`. |
| `WVST_ALLOWED_ORIGINS` | Comma-separated origin allowlist. |
| `WVST_ALLOW_LOOPBACK_ORIGINS` | Set to `0` or `false` to stop auto-allowing loopback origins. |
| `WVST_HOST_WORKER` | Override host worker executable discovery. |
| `WVST_WORKER_AUTO_RESTART` | Set to `0` or `false` to disable automatic worker restart. |
| `WVST_MAX_WORKER_INSTANCES` | Cap concurrent worker instances. Default is `64`. |
| `WVST_WORKER_QUARANTINE_FAILURES` | Failures before a plugin enters quarantine. Default is `3`. |
| `WVST_MAX_CONTROL_MESSAGE_BYTES` | Cap JSON control message size. |
| `WVST_WORKER_MEMORY_LIMIT_BYTES` | Address-space limit for supervised workers where supported. |
| `WVST_WORKER_CPU_TIME_LIMIT_SECONDS` | CPU time limit for supervised workers where supported. |
| `WVST_WORKER_LINUX_CGROUP_PARENT` | Linux cgroup parent for worker supervision. |
| `WVST_WORKER_LINUX_CGROUP_MEMORY_MAX_BYTES` | Linux cgroup memory max. |
| `WVST_WORKER_LINUX_CGROUP_CPU_QUOTA_MICROS` | Linux cgroup CPU quota. |
| `WVST_WORKER_LINUX_CGROUP_CPU_PERIOD_MICROS` | Linux cgroup CPU period. |

Example:

```sh
WVST_TOKEN=dev-token \
WVST_ALLOWED_ORIGINS=http://127.0.0.1:5173 \
cargo run -p wvst-bridge-server
```

## Run the Docs Site

```sh
npm run docs:dev
```

The docs dev server first builds `@wvst/web`, then starts Svedocs. The project Vite config sends:

```txt
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Those headers are also applied from SvelteKit hooks so production-style local builds keep the same low-latency prerequisites.

## Minimal Web SDK Flow

This is the smallest control-plane path for a real plugin instance:

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

`client.hello` contains negotiated protocol and Bridge metrics. `client.metrics()` and `client.events()` can be called after connection.

## Add the AudioWorklet Path

The browser live path uses an AudioWorklet node plus a DedicatedWorker audio pump:

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

Call `readLoopbackMetrics(buffers)` to display pending input/output quanta, underflows, overflows, dropped events, late events, and transport failures.

## Parameter and State Basics

After an instance is ready:

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

Use `instanceStateSnapshotToSetStateOptions()` to restore a snapshot into a compatible instance.

## MIDI Basics

Instrument plugins and MIDI-capable effects can receive events through the bridge worker:

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

The helper validates channel, velocity, pitch bend, and sample offset. Web MIDI can be adapted with `createWVSTWebMidiAdapter()`.

## Run Checks

```sh
npm run docs:check
npm run docs:build -- --no-og
```

For the whole repository:

```sh
npm run check
```

`npm run check` runs Rust tests and the Web SDK type check.
