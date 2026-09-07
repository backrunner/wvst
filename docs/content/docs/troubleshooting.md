---
title: Troubleshooting
description: Diagnose browser isolation, Bridge authorization, plugin discovery, worker lifecycle, shared-memory transport, and realtime metrics.
order: 8
---

# Troubleshooting

Locate the failing layer before changing settings. SDK snippets assume imported helpers and a current application session providing `client`, `instanceId`, `buffers` and related objects.

| Symptom | First check |
| --- | --- |
| Startup reports a missing token | Set nonempty `WVST_TOKEN` for the standalone CLI. |
| Socket cannot connect | Running Bridge, matching port and browser local-network access. |
| Scanning works but audio keeps failing | Separate authenticated handshake on the audio Worker. |
| Discovered plugin cannot mount | Worker executable, plugin CPU architecture, class ID and VST3 buses. |
| Original audio works, effect is silent | Instance state, stream configuration, audio error counters and plugin capabilities. |
| Localhost works, deployed site fails | Final COOP/COEP response, asset URLs and actual page origin. |

The two-terminal setup in [Getting started](/docs/getting-started) provides a useful baseline.

## SharedArrayBuffer Is Unavailable

Check in the browser:

```ts
WVSTClient.lowLatencyPrerequisites();
```

Both values must be true:

- `sharedArrayBuffer`
- `crossOriginIsolated`

Serve the app with:

```txt
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Use `npm run docs:dev` for the local docs site. A page can be on localhost and still fail if COOP/COEP headers are missing.

## Bridge Connection Fails

Start the Bridge:

```sh
cargo build -p wvst-bridge-server -p wvst-host-worker
WVST_TOKEN=local-dev-token \
WVST_HOST_WORKER=target/debug/wvst-host-worker \
  target/debug/wvst-bridge-server serve
```

Default endpoint:

```txt
ws://127.0.0.1:35876
```

If the endpoint is different, set `WVST_BIND_ADDR` before starting the Bridge and enter the matching WebSocket URL in the app.

Run diagnostics:

```sh
cargo run -p wvst-bridge-server -- diagnose
```

## Session Not Authorized

The standalone CLI requires a nonempty `WVST_TOKEN`. Control methods other than `bridge.hello`, and binary audio, need an authorized connection. Pass the same token to `WVSTClient.connect({ token })`. The audio Worker owns another socket and must complete its own `bridge.hello`.

If origin checks fail:

- Add the exact origin to `WVST_ALLOWED_ORIGINS`.
- Keep `WVST_ALLOW_LOOPBACK_ORIGINS=true` for local development.
- Avoid wildcard origins for strong plugin permissions.

## No Plugins Appear

Run a rescan:

```ts
await client.plugins.scan();
```

On macOS, the scanner uses standard VST3 paths such as:

- `/Library/Audio/Plug-Ins/VST3`
- `~/Library/Audio/Plug-Ins/VST3`

You can pass explicit paths:

```ts
await client.plugins.scan({
  paths: ["/Library/Audio/Plug-Ins/VST3"]
});
```

For deeper metadata, call:

```ts
await client.plugins.factoryInfo({ path: "/path/to/Plugin.vst3" });
```

Factory info is queried through the host worker, not by loading VST3 code inside the Bridge Server.

## Instance Creation Fails

Confirm the `pluginId` and optional `classId` came from the latest scan report. Use the actual `sampleRate` from the `AudioContext` and choose a realistic block size:

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

Unsupported bus arrangements usually show up as worker errors or failed runtime capabilities. Check:

```ts
await client.instances.status({ instanceId });
await client.instances.runtimeSnapshot({ instanceId, includeRecentEvents: true });
```

## Mounted Slot Is Silent

Check the layers in order:

1. Browser media element is playing.
2. The WebAudio source is connected to the active slot chain.
3. The instance is in `processing` state.
4. The stream is `open`.
5. The Bridge worker audio stream was started with matching `streamId`, `sampleRate`, frames, and channel counts.
6. `readLoopbackMetrics(buffers)` is not continuously increasing `underflows` or `transportFailures`.
7. `client.metrics()` does not show route failures or unmatched streams.

The current live rack is optimized for 2-in/2-out effects. Instruments can be driven through MIDI APIs, but they are not the first live demo workflow.

## AudioWorklet Processor Registration Fails

Browsers reject duplicate processor names in one `AudioContext`. Load `@wvst/web/loopback-processor` once and reuse that promise:

```ts
workletModule ??= audioContext.audioWorklet.addModule(loopbackProcessorUrl);
await workletModule;
```

Then create new `AudioWorkletNode` objects for each rack slot.

## Metrics Show Underflows or Overflows

Loopback metrics are browser-side counters:

- `underflows`: output was not ready when the worklet needed it.
- `overflows`: input or event queues exceeded capacity.
- `droppedInputQuanta` / `droppedOutputQuanta`: ring pressure.
- `droppedMidiEvents` / `droppedParameterEvents`: event queue pressure.
- `lateMidiEvents` / `lateParameterEvents`: event target sequence already passed.
- `transportFailures`: DedicatedWorker or Bridge transport failed.

Bridge metrics add server-side context:

```ts
const metrics = await client.metrics();
```

Look at route failures, invalid frame headers/lengths, unmatched streams, sequence gaps, duplicate/out-of-order/late frames, backpressure drops, shared-memory pump errors, and latency histograms.

## Worker Failed or Was Quarantined

Listen to Bridge events:

```ts
const unsubscribe = client.onEvent((event) => {
  console.log(event.kind);
});
```

Important worker events:

- `worker-failed`
- `worker-recovering`
- `worker-recovered`
- `worker-recovery-failed`
- `worker-quarantined`
- `worker-quarantine-released`
- `worker-policy-decision`

Automatic restart is controlled by `WVST_WORKER_AUTO_RESTART`. Repeated failures are limited by `WVST_WORKER_QUARANTINE_FAILURES`.

If the worker never launches, run `cargo run -p wvst-bridge-server -- diagnose` and check `WVST_HOST_WORKER`, executable permissions, CPU architecture, and the local package layout.

## Metadata Changed While Running

Some VST3 plugins emit component handler restart or metadata invalidation events. Use:

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

If the refresh policy is `rebuild-audio-graph` or `reload-component`, the app should rebuild the affected graph or recreate the instance.

## State Restore Fails

WVST stores VST3 component/controller state as opaque base64. Do not inspect or edit plugin-private state.

Before restore:

```ts
const compatibility = checkWVSTInstanceStateSnapshotCompatibility(
  snapshot,
  descriptor
);
```

Compatibility checks plugin id, class id, sample rate, block size, and channel counts when those fields are present in the snapshot.

## MIDI Does Not Trigger an Instrument

Check:

- The instance has `inputChannels: 0` or an arrangement valid for the instrument.
- The worker runtime capabilities include `midiMapping` when relying on CC/pitch bend mapping.
- `sampleOffset` is below the block frame count.
- The Bridge worker stream is active before `sendMidiEvents`.

For keyboard testing:

```ts
const keyboard = createWVSTVirtualKeyboard(target);
await keyboard.noteOn(60, 0.9);
await keyboard.noteOff(60);
```

## Shared-Memory Pump Does Not Run

Check the data-plane snapshot:

```ts
await client.instances.runtimeSnapshot({
  instanceId,
  includeRecentEvents: true
});
```

The snapshot includes:

- `dataPlane.sharedMemory`
- `dataPlane.sharedMemoryPump`

Common issues are missing shared-memory descriptor, pump already stopped, input underrun, output backpressure, worker error, event queue overflow, or an unavailable platform mmap backend.

## Stability and Packaging Evidence

Use testkit tools when validating a machine or package:

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

The `.agents/*.example.json` files provide starting fixtures for these reports.


## Control Works but Audio Is Unauthorized

`WVSTBridgeWorkerClient.connect()` only opens a WebSocket. Follow it with:

```ts
await bridgeWorker.request('bridge.hello', {
  ...client.createHelloRequest().params,
  token: tokenFromUser
});
```

`createHelloRequest()` does not retain the original connection token; add it explicitly. See [Web integration](/docs/web-integration) for the full flow.

## File, Playback and Reconnection Issues

If autoplay is rejected, call `audioContext.resume()` in the user's play handler before media `play()`. Use the built-in loop or WAV to rule out codec problems. Studio skips waveform decoding above 50 MB to control memory use.

Do not create another `MediaElementAudioSourceNode` for an existing media element. Reuse the source when reconnecting and release it when disposing the element and complete graph. Closed WebSockets require new control/Worker connections, repeated handshakes and newly mounted instances.

## Deployed Assets or Local-Network Access Fail

If Worker or worklet requests return `text/html`, inspect SPA fallback and asset URLs. The final page response must contain COOP/COEP; static pages do not execute SvelteKit server hooks.

If isolation succeeds but the socket is blocked, inspect console errors for mixed content, local-network permissions and origin policy. Changing `ws://` to `wss://` alone does not enable TLS on the Bridge CLI. See [Configuration and deployment](/docs/configuration).

## Capture Useful Diagnostics

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

Keep counter deltas around the failure, reproduction steps, browser/OS versions and plugin version. Remove tokens and unnecessary personal paths before sharing. See [Development](/docs/development) for real-plugin and sustained-audio validation.
