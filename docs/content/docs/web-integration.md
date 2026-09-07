---
title: Web Integration
description: Authenticate control and audio connections in a Vite app, mount a stereo VST3 effect, and release every owned resource.
order: 5
---

# Web Integration

Connect one local stereo VST3 effect to your application's existing `AudioContext`. First complete [Getting started](/docs/getting-started) and verify the Bridge/plugin in Studio, then work on your own integration.

## Bundling and browser prerequisites

Run `npm run build:web` in this repository and consume `@wvst/web` through a workspace dependency. The package is currently private; `npm install @wvst/web` is not a published installation path.

The example uses Vite's `?worker` and `?url` imports. TypeScript apps need `vite/client` types. Other bundlers must emit an independent Worker module and serve the AudioWorklet entry as a JavaScript URL. Create neither a Worker nor an AudioContext during server rendering.

The page needs a secure context, AudioWorklet, SharedArrayBuffer and cross-origin isolation. `requireLowLatency: false` only skips the SDK check; it does not provide a non-SAB audio fallback. See [Configuration and deployment](/docs/configuration) for headers.

## Let the user choose a plugin

Use `client.plugins.list({ rescan: true })` to obtain both `plugins` and `failures`. Present plugin classes and obtain the selected `pluginId` and `classId`; do not assume `plugins[0]` or `classes[0]` exists.

If scan metadata lacks a class ID, query `client.plugins.factoryInfo({ path })` and let the user select an audio component class. Discovery does not establish 2-in/2-out compatibility. Preserve mount errors so the user can try a compatible effect.

## Mount one effect

Save this as `mount-effect.ts` in a Vite app. The caller owns the source and AudioContext; the helper owns its connections, instance and worklet. Disconnect any direct source-to-destination edge before mounting, or dry and wet signals will both play.

The control socket and audio-worker socket each complete `bridge.hello`. `createHelloRequest()` does not retain the original token, so explicitly add it to the worker handshake.

```ts
import {
  WVSTClient,
  WVSTBridgeWorkerClient,
  configureLoopbackAudioWorkletNode,
  createLoopbackSharedBuffers,
  readLoopbackMetrics,
  type InstanceDescriptor
} from '@wvst/web';
import BridgeWorker from '@wvst/web/bridge-worker?worker';
import processorUrl from '@wvst/web/loopback-processor?url';

const workletModules = new WeakMap<AudioContext, Promise<void>>();

export async function mountEffect(
  context: AudioContext,
  source: AudioNode,
  options: { pluginId: string; classId: string; token: string; endpoint?: string }
) {
  const endpoint = options.endpoint ?? 'ws://127.0.0.1:35876';
  let client: WVSTClient | undefined;
  let worker: Worker | undefined;
  let transport: WVSTBridgeWorkerClient | undefined;
  let instance: InstanceDescriptor | undefined;
  let node: AudioWorkletNode | undefined;
  let connected = false;
  let disposePromise: Promise<void> | undefined;

  function dispose(): Promise<void> {
    return disposePromise ??= (async () => {
      const errors: unknown[] = [];
      const attempt = async (operation: () => unknown) => {
        try { await operation(); } catch (error) { errors.push(error); }
      };
      if (connected && node) await attempt(() => source.disconnect(node!));
      if (node) {
        await attempt(() => node!.disconnect());
        node.port.close();
      }
      if (instance) {
        const { instanceId, streamId } = instance;
        if (transport) await attempt(() => transport!.stopAudioStream(streamId));
        if (client) {
          await attempt(() => client!.instances.stop({ instanceId }));
          await attempt(() => client!.instances.closeStream({ instanceId }));
          await attempt(() => client!.instances.destroy({ instanceId }));
        }
      }
      if (transport) await attempt(() => transport!.close());
      worker?.terminate();
      client?.close();
      if (errors.length) throw new AggregateError(errors, 'WVST cleanup failed');
    })();
  }

  try {
    client = await WVSTClient.connect({
      endpoint, token: options.token, requireLowLatency: true
    });
    worker = new BridgeWorker();
    transport = new WVSTBridgeWorkerClient({ worker });
    await transport.connect(endpoint);
    await transport.request('bridge.hello', {
      ...client.createHelloRequest().params,
      token: options.token
    });

    instance = await client.instances.create({
      pluginId: options.pluginId,
      classId: options.classId,
      sampleRate: Math.round(context.sampleRate),
      maxBlockFrames: 128,
      inputChannels: 2,
      outputChannels: 2
    });
    await client.instances.start({ instanceId: instance.instanceId });

    const buffers = createLoopbackSharedBuffers({
      frames: 128, inputChannels: 2, outputChannels: 2, capacityQuanta: 4
    });
    let module = workletModules.get(context);
    if (!module) {
      module = context.audioWorklet.addModule(processorUrl);
      workletModules.set(context, module);
      void module.catch(() => workletModules.delete(context));
    }
    await module;
    node = new AudioWorkletNode(context, 'wvst-loopback', {
      numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [2],
      channelCount: 2, channelCountMode: 'explicit'
    });
    configureLoopbackAudioWorkletNode(node, buffers);
    await transport.startAudioStream({
      streamId: instance.streamId, sampleRate: instance.sampleRate,
      frames: 128, inputChannels: 2, outputChannels: 2, buffers
    });
    source.connect(node);
    connected = true;
    node.connect(context.destination);

    return { instance, readMetrics: () => readLoopbackMetrics(buffers), dispose };
  } catch (error) {
    try { await dispose(); }
    catch (cleanupError) {
      throw new AggregateError([error, cleanupError], 'WVST setup and cleanup failed');
    }
    throw error;
  }
}
```

## Playback and ownership

Call `context.resume()` in the user's play handler before starting the media or source, to satisfy browser autoplay policy. This helper does not create media elements, request microphone access or close an application-owned AudioContext.

Keep the result in your component:

```ts
const effect = await mountEffect(context, source, {
  pluginId: selectedPluginId,
  classId: selectedClassId,
  token: tokenFromUser
});

// Read from a UI timer, never log inside AudioWorklet.process().
console.table(effect.readMetrics());

// Remove the effect or leave the page; handle remote cleanup errors.
await effect.dispose();
```

This continues the helper above; your app supplies the context, source, selection and token. Validate the path using Studio's built-in sound before substituting your own media input.

| Resource | Caller responsibility |
| --- | --- |
| AudioContext | Resume from user interaction; close only when the whole audio session ends. |
| Media element and source | Create one `MediaElementAudioSourceNode` per media element and reuse it on reconnect. |
| File URL | Call `URL.revokeObjectURL()` when replacing a file or unmounting. |
| Effect session | Partial setup failures clean up; normal removal must await `dispose()`. |
| Subscriptions and timers | Unsubscribe and stop timers so destroyed components no longer update. |

`dispose()` can be called repeatedly. It attempts every cleanup step and aggregates failures. Remote requests can fail after the Bridge exits; your app should still remove local audio connections. If the component unmounts during setup, dispose the session as soon as mounting finishes, and prevent concurrent mount operations.

## Multiple effects and reconnecting

This helper demonstrates one effect. A rack should share a control client and transport Worker while each slot owns its instance, buffers, node and stream. Construct `source → effect A → effect B → destination`; disconnect old edges before changing the graph to prevent duplicate routes.

Bypass removes a node from the signal path; it does not necessarily stop its plugin worker or eliminate its CPU use. Removal releases the instance and stream. Studio's `rack-demo/audio-graph.ts` and `WVSTRackDemo.svelte` provide the complete example.

After closure, create a new control client and Worker and repeat both handshakes. Do not reuse closed sockets or assume old instances are reusable. Save state snapshots ahead of time if recovery should restore parameters, and check plugin, class and processing configuration compatibility.

## Channels, timing and metrics

The example uses 128 frames, 2-in/2-out and the actual AudioContext sample rate. Four quanta describe ring capacity, not fixed delay or delay compensation. At 48 kHz, 128 frames are about 2.67 ms; the round trip additionally includes scheduling, transport, queues and plugin latency.

`readMetrics()` returns queues, underflows/overflows, dropped events and transport failures, not end-to-end latency. Compare counter deltas over time so a startup underflow is not mistaken for persistent failure. See [API reference](/docs/api-reference) for metric sources.

Instruments need a compatible zero-input or other bus arrangement and sample-offset MIDI events. Changing this effect example's input channels to zero alone does not establish instrument playback. See `packages/wvst-web-examples/src/instrument` and [Troubleshooting](/docs/troubleshooting).
