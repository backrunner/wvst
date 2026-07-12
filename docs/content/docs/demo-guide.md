---
title: Demo Guide
description: Run the WVST browser rack with the local bridge and a real VST3 effect.
order: 4
---

# Demo Guide

The standalone [Demo](/demo) is a real browser-to-native audio path. It loads an audio file, connects to the local Bridge Server, discovers VST3 metadata, and routes audio through isolated worker instances.

## Browser Requirements

- Serve the site from localhost or HTTPS.
- Send `Cross-Origin-Opener-Policy: same-origin`.
- Send `Cross-Origin-Embedder-Policy: require-corp`.
- Confirm `crossOriginIsolated` and `SharedArrayBuffer` are available.

The demo reports a clear failure when a prerequisite is missing. It never falls back to fake playback.

## Start The Bridge

#### Download a release

Use the [WVST GitHub Releases](https://github.com/backrunner/wvst/releases) page to download the platform package. It includes the Bridge Server, the isolated host worker, and the service installer.

No release binary is published yet. For now, build both binaries from the repository root:

```sh
cargo build --release -p wvst-bridge-server -p wvst-host-worker
WVST_HOST_WORKER=target/release/wvst-host-worker \
  target/release/wvst-bridge-server serve
```

Keep the process or installed service running, then open the [Demo](/demo). The page tries the default `ws://127.0.0.1:35876` endpoint once on load. Use Connect to retry after changing the endpoint or token.

The first connection negotiates the Bridge version, origin policy, optional token, and available plugin capabilities.

## Rack Lifecycle

Each mounted slot owns one WVST instance, one shared-buffer allocation, one `AudioWorkletNode`, and one Bridge worker stream. The browser keeps the realtime thread non-blocking while the worker handles transport.

When a slot is removed, the rack disconnects the worklet, stops the stream, stops processing, closes the stream, destroys the instance, and rebuilds the remaining graph.

## What To Watch

The rack exposes pending quanta, underflow, overflow, dropped events, worker restarts, and transport failures. Use these counters to distinguish a plugin failure from a browser or bridge setup problem.

For the public method and control-plane surface, see the [API Reference](/docs/api-reference).
