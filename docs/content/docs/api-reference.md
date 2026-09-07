---
title: API Reference
description: Web SDK connection options, instances and parameters, state snapshots, audio workers, MIDI, metric sources and errors.
order: 6
---

# API Reference

Public types are exported by `@wvst/web`; building generates complete declarations in `packages/wvst-web/dist/types`. This page explains the main interfaces by task. See [Web integration](/docs/web-integration) for a complete audio helper. Snippets share a connected `client`; instance examples assume an already selected/created `instance` or `instanceId`.

## Connection and handshake

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

| `ConnectOptions` field | Default | Purpose |
| --- | --- | --- |
| `endpoint` | `ws://127.0.0.1:35876` | Local WebSocket URL. |
| `clientName` | `@wvst/web` | Client identity for diagnostics. |
| `clientVersion` | `0.1.0` | App version; protocol negotiation is separate. |
| `token` | Unset | Must match `WVST_TOKEN` for the standalone Bridge CLI. |
| `requireLowLatency` | `true` | Check SAB and isolation before connecting. Disabling skips the check, not the need for an audio implementation. |
| `transportOptions` | SDK defaults | WebSocket handshake, control/audio deadlines and pending-request cap; see below. |

`WVSTClient.lowLatencyPrerequisites()` returns two booleans: `sharedArrayBuffer` and `crossOriginIsolated`. Apps should also check `isSecureContext` and AudioWorklet. `client.hello` holds negotiated protocol/audio-frame versions, Bridge identity and capability requirements. Ordinary RPC responses do not necessarily repeat version fields.

`client.close()` closes control transport; it does not replace instance destruction, graph disconnection or Worker termination.

`transportOptions` accepts `connectTimeoutMs` (10,000), `requestTimeoutMs` (180,000), `binaryTimeoutMs` (10,000) and `maxPendingRequests` (256), all positive 32-bit integers. A deadline closes that socket and rejects all its pending requests, so a late binary response cannot be assigned to the next request. These are failure deadlines, not acceptable realtime latency targets. `WVSTBridgeWorkerClient.connect(endpoint, transportOptions?)` configures the separate audio socket. Keep control and audio on separate connections; pause audio processing before a long state restore. When increasing `WVST_WORKER_LOAD_TIMEOUT_MS`, also leave sufficient headroom in `requestTimeoutMs`. Closing the browser socket does not cancel native loading or destroy an instance.

## Plugin discovery

| Method | Result or purpose |
| --- | --- |
| `client.plugins.list({ rescan, paths })` | Return `PluginScanReport`, optionally rescanning. |
| `client.plugins.scan({ paths })` | Scan default or explicit paths. |
| `client.plugins.factoryInfo({ path })` | Query factory/classes inside a host worker. |

`PluginScanReport` contains `plugins` and `failures`; scans can partially succeed. Descriptors include `pluginId`, `path`, `format`, name, optional vendor/version, `metadataSource` and `classes`. Static metadata can lack class IDs and require a factory query. Handle empty arrays and failures before presenting a selection.

## Instance and stream lifecycle

| SDK method | Behavior |
| --- | --- |
| `instances.create(options)` | Return an `InstanceDescriptor` with distinct `instanceId` and `streamId`. |
| `instances.start({ instanceId })` | Start plugin processing. |
| `instances.status({ instanceId })` | Query current instance/worker state. |
| `instances.stop({ instanceId })` | Stop processing while retaining the instance. |
| `instances.restart({ instanceId })` | Request worker recovery; recheck state and graph connections afterward. |
| `instances.openStream({ instanceId })` | Reopen a closed stream. |
| `instances.closeStream({ instanceId })` | Close the associated stream. |
| `instances.destroy({ instanceId })` | Release the native instance. |

Core creation options are `pluginId`, `classId`, `sampleRate`, `maxBlockFrames`, `inputChannels` and `outputChannels`, with bus selection options in the types. Use the actual AudioContext sample rate. Newly created instances normally have an open stream, but processing still needs `start()`.

Control APIs use `instanceId`; audio-worker stream commands use `streamId`. A returned descriptor is a snapshot and does not update itself. Refresh through `status()` or `runtimeSnapshot()`.

## Parameter editing

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

Normalized values use `0..1`; do not pass display units such as Hz or dB into `valueNormalized`. `parameterInfo()` provides supported plain/display conversions. Respect `stepCount` for discrete parameters and filter hidden/read-only flags.

`parameterEdit()` aggregates begin/perform/end gestures for a single UI edit. A drag can use `parameterBeginEdit()`, `parameterPerformEdit()` and `parameterEndEdit()` separately. For sample-offset automation, enqueue `sendParameterEvents()` through the audio Worker instead of treating UI timers as sample-accurate scheduling.

## Save and restore state

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

Both `instance` and `target` are created instance descriptors. Your application persists the JSON. Validate schema and fields when reading a file; a TypeScript assertion does not validate external data. The helper checks supported schema, restorable content and compatibility with the supplied target configuration.

Component/controller state is opaque plugin-owned base64 data. Snapshot checks cannot guarantee compatibility across plugin versions. Use returned metadata to refresh the UI after restore. Do not report a fabricated empty-state success for a plugin without state support.

## AudioWorklet and transport worker

| Interface | Responsibility |
| --- | --- |
| `createLoopbackSharedBuffers(options)` | Allocate input/output SABs and counters; default capacity is four quanta. |
| `configureLoopbackAudioWorkletNode(node, buffers)` | Send shared-buffer configuration to an existing node. |
| `createLoopbackAudioWorkletNode(context, options)` | Load a processor and create a node; manage module loading centrally for multiple nodes. |
| `WVSTBridgeWorkerClient.connect(endpoint, transportOptions?)` | Open the worker socket; then authorize with `request('bridge.hello', params)`. |
| `startAudioStream(options)` | Start transport using matching stream ID, sample rate, frames, channels and buffers. |
| `stopAudioStream(streamId)` | Stop that browser audio stream. |
| `close()` | Close worker transport; the owner of the native Worker must also call `terminate()`. |

Register `wvst-loopback` once per AudioContext. See [Web integration](/docs/web-integration) for both handshakes and cleanup order.

## MIDI, devices and native shared memory

`createWVSTVirtualKeyboard()` constructs note, CC, pitch bend and aftertouch events; `createWVSTWebMidiAdapter()` converts Web MIDI messages. Events carry channel and `sampleOffset`, which must fit the block. Send through `sendMidiEvents({ streamId, events })` after starting the stream. Browser support and permissions govern Web MIDI availability.

`createWVSTAudioDeviceSession()` manages device input/output routing; browser/user permission is still required. `createWVSTSharedMemoryPumpSession()` manages Bridge-side file-mapped shared memory. That is separate from browser SharedArrayBuffer storage; a browser does not directly map the native files.

## Where metrics come from

| Source | What it describes |
| --- | --- |
| `readLoopbackMetrics(buffers)` | Input/output frames, pending quanta, underflows/overflows, dropped/late events and transport failures. |
| `client.metrics()` | Bridge routing, sequence anomalies, jitter/route latency histograms and shared-memory pump metrics. |
| `instances.status()` / `runtimeSnapshot()` | Plugin-reported `latencySamples`, runtime capabilities and worker/data-plane state. |
| `client.events()` / `onEvent()` | Worker failure, recovery, quarantine and metadata lifecycle events. |
| External loopback measurements and testkit | Observations used for end-to-end round-trip percentiles and stability budgets. |

`latencySamples / sampleRate * 1000` converts only plugin-declared latency, excluding browser and transport. `capacityQuanta` is not a fixed delay. CPU and audio-health claims require measurements and cannot be inferred from queue depth.

## Errors and subscriptions

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

Control-client RPC errors use `WVSTBridgeError` with `code`, `message` and optional `data`. Connection/prerequisite errors use `Error`. The worker wrapper returns failures as error messages; do not assume it preserves `WVSTBridgeError` structured fields.

`onEvent()` returns an unsubscribe function. Catch failures in async metadata refresh, or use `onMetadataInvalidated(listener, { onError })`. When a refresh policy requires graph reconstruction or component reload, your app must handle that lifecycle; fetching metadata does not rebuild the graph.

## Common control-plane mappings

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

For advanced units, program data, connection notifications and event payloads, consult the exported types and [Architecture](/docs/architecture).


### Web MIDI lifecycle and limits

The adapter follows device hot-plug events and accepts `inputIds` to select inputs. Call `await adapter.stop()` before stopping the audio session; it removes listeners and sends releases for tracked notes and sustain/sostenuto/hold pedals. `await adapter.panic()` sends releases while keeping the inputs connected. Supply `onError` to display malformed-message and queue/transport failures.

The worker acknowledges MIDI enqueue, not native processing. Keep audio processing active for releases to reach the plugin; immediately closing the stream can discard them. There is currently no public MIDI flush acknowledgement. When ending a session, also stop/destroy its native instance. Neither panic nor stop guarantees release delivery after a queue or transport failure.

CC, pitch bend and channel aftertouch require the plugin's VST3 `IMidiMapping`. Program Change and system messages are not implemented by the native input converter; raw transport does not imply plugin support. SysEx and incomplete or concatenated short messages are rejected. `sampleOffset` is an explicit offset in the next block, not automatic conversion of Web MIDI timestamps. Overdue MIDI is applied at offset 0 in its original order to preserve note-off and pedal releases.
