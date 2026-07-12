---
title: API Reference
description: Public Web SDK methods, realtime helpers, Bridge operations, and runtime error contracts.
order: 5
---

# API Reference

This reference describes the public surface currently used by the examples and the standalone demo. Types are exported from `@wvst/web`; control methods are sent to the local Bridge Server over the negotiated transport.

## Connect

```ts
import { WVSTClient } from '@wvst/web';

const client = await WVSTClient.connect({
  endpoint: 'ws://127.0.0.1:35876',
  clientName: 'my-wvst-app',
  clientVersion: '0.1.0',
  requireLowLatency: true
});
```

`connect()` checks the secure context, cross-origin isolation, token policy, and Bridge protocol version. When low-latency prerequisites are unavailable, it throws before opening the transport instead of silently changing modes.

## Plugins

```ts
const report = await client.plugins.list({ rescan: false });
await client.plugins.scan();
const factory = await client.plugins.factoryInfo({
  path: report.plugins[0].path
});
```

`PluginScanReport` contains discovered plugin descriptors and scan failures. Descriptors expose the plugin id, format, path, metadata source, vendor, version, and classes. Runtime buses, parameters, units, programs, latency, and tail become available after instance creation.

## Instances

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

The default isolation policy gives every instance its own host worker. A worker crash is reported to the Bridge and does not take down the Bridge process.

## Audio Nodes

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

The AudioWorklet exchanges bounded buffers with a `WVSTBridgeWorkerClient`. It never waits for native processing, performs network I/O, or parses JSON in `process()`. See [Getting Started](/docs/getting-started#add-the-audioworklet-path) for the complete worker stream setup.

## Parameters And State

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

For gesture-aware automation use `parameterBeginEdit`, `parameterPerformEdit`, and `parameterEndEdit`, or the aggregate `parameterEdit`. Restore persisted state with `instanceStateSnapshotToSetStateOptions()` and `client.instances.setState()`. Plugin state remains opaque base64 data.

## MIDI And Devices

The package exports `createWVSTVirtualKeyboard()`, `createWVSTWebMidiAdapter()`, and `createWVSTAudioDeviceSession()`. Note, CC, pitch bend, aftertouch, and raw MIDI events carry channel and sample-offset information.

## Shared Memory And Metrics

Use `createLoopbackSharedBuffers()`, `configureLoopbackAudioWorkletNode()`, and `readLoopbackMetrics()` for the browser loopback path. Metrics include configured latency, round-trip time, jitter, underflows, overflows, late frames, worker restarts, plugin latency, and process CPU.

## Bridge Operations

The control plane uses versioned JSON-RPC messages. Common methods include:

- `bridge.hello`, `bridge.metrics`, `bridge.events`
- `plugin.scan`, `plugin.list`, `plugin.factoryInfo`
- `instance.create`, `instance.list`, `instance.status`, `instance.start`, `instance.stop`, `instance.restart`, `instance.destroy`
- `instance.parameters`, `instance.parameter.get`, `instance.parameter.info`, `instance.parameter.beginEdit`, `instance.parameter.performEdit`, `instance.parameter.endEdit`, `instance.parameter.edit`
- `instance.getState`, `instance.setState`, `instance.state.setAndRefresh`
- `stream.open`, `stream.close`, `stream.sharedMemory.create`, `stream.sharedMemory.process`, `stream.sharedMemory.pump.start`, `stream.sharedMemory.pump.status`

Every response includes a protocol or Bridge version so a client can diagnose mismatches.

## Errors

Bridge JSON-RPC failures reject with `WVSTBridgeError`, which exposes a numeric `code`, a human-readable `message`, and optional structured `data`. WebSocket connection failures and missing low-latency browser prerequisites reject with `Error` before an RPC session exists.

Bridge events report worker exits, recovery, quarantine, stream state, metrics, and VST3 metadata invalidation. Subscribe with `client.onEvent()` and close the returned unsubscribe function during teardown.
