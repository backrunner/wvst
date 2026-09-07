---
title: Getting Started
description: Build the Bridge from source, process your first local VST3 effect in Live Studio, and verify each layer.
order: 2
---

# Getting Started

Start with the repository's Live Studio. You need a browser, the Rust Bridge, a host worker and a VST3 effect on the same computer. Even when the webpage is hosted remotely, the Bridge runs on the user's machine.

## Prepare your environment

| Component | Requirement and check |
| --- | --- |
| Node.js | Use 22; check with `node --version`. |
| Rust | Use stable from `rust-toolchain.toml`; inspect it with `rustc --version`. |
| macOS build tools | `xcode-select -p` should print the developer tools path. Install with `xcode-select --install` if needed. |
| Browser | Start with current desktop Chromium, with AudioWorklet, secure context and cross-origin isolation. |
| VST3 | Install a stereo input/output effect matching the host worker's CPU architecture. An AU-only installation is insufficient. |

macOS is the first runtime target. Responsive mobile styling does not mean a phone can host desktop VST3 plugins. Windows/Linux plugin compatibility requires separate validation.

## 1. Install workspace dependencies

```sh
git clone https://github.com/backrunner/wvst.git
cd wvst
npm ci
```

Run subsequent commands from the repository root. `@wvst/web` is a local workspace package; no separate npm installation or adjacent Svedocs checkout is required.

## 2. Build and start the Bridge

Build both executables:

```sh
cargo build -p wvst-bridge-server -p wvst-host-worker
```

In terminal A:

```sh
WVST_TOKEN=local-dev-token \
WVST_HOST_WORKER=target/debug/wvst-host-worker \
  target/debug/wvst-bridge-server serve
```

Expect:

```text
wvst-bridge-server listening on ws://127.0.0.1:35876
```

Keep terminal A running. The standalone CLI requires a nonempty `WVST_TOKEN`. This value is a local development example; keep real tokens out of frontend bundles and source control. `WVST_HOST_WORKER` points at the worker you built; building only the Bridge does not produce that executable.

For an optimized build, use a matching pair of release binaries:

```sh
cargo build --release -p wvst-bridge-server -p wvst-host-worker
WVST_TOKEN=local-dev-token \
WVST_HOST_WORKER=target/release/wvst-host-worker \
  target/release/wvst-bridge-server serve
```

No release binaries are published yet. Check [Releases](https://github.com/backrunner/wvst/releases) for future availability; use source builds now.

## 3. Start the docs and Studio

In terminal B:

```sh
npm run docs:dev
```

This builds the Web SDK and starts Svedocs. Open the printed address (normally `http://localhost:5173`) and visit [Live Studio](/demo). If the port is occupied, use the actual URL printed by the server.

The dev server supplies:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Run this in the page console; all three values should be `true`:

```js
({
  secureContext: window.isSecureContext,
  crossOriginIsolated: window.crossOriginIsolated,
  sharedArrayBuffer: typeof SharedArrayBuffer === 'function'
})
```

A secure localhost page is not automatically cross-origin isolated. Do not open the HTML as a file or replace the server with one that omits these headers. For hosting, see [Configuration and deployment](/docs/configuration).

## 4. Hear your first effect

1. Expand advanced connection settings. Enter `ws://127.0.0.1:35876` and token `local-dev-token`, then connect. The initial automatic attempt has no token; fill it in if authorization fails.
2. Choose **Try a synth loop** or drop a browser-supported audio file. Preview original audio first to verify your output device.
3. Select and mount a VST3 effect. If the list is empty, rescan and inspect scan failures.
4. Press Play, adjust parameters and compare with bypass. Confirm the effect is enabled in the signal path.
5. Expand **Processing details** to inspect queues and error counters. Output meters respond to actual audio.

macOS scan paths include `/Library/Audio/Plug-Ins/VST3` and `~/Library/Audio/Plug-Ins/VST3`. Discovery does not establish channel-layout, licensing or processing compatibility.

## 5. Stop and reconnect

Pause playback, then remove effects or disconnect the Bridge. Studio releases its instances and audio streams. Remount effects after reconnecting. Finally, press Ctrl+C in terminal A to stop the Bridge and in terminal B to stop the docs server.

Refreshing the page does not preserve the rack or plugin state. Application persistence requires the SDK's state snapshot APIs.

## First diagnostic checkpoints

| Symptom | Next check |
| --- | --- |
| Bridge does not start | Token, occupied port and executable paths. |
| Browser cannot connect | Same computer, matching endpoint and browser local-network restrictions. |
| Scan succeeds but mount fails | Worker path, plugin architecture, class ID and channel layout. |
| Mounted effect is silent | Bypass to verify the source, then check instance state, audio-worker authorization and counters. |

Inspect local diagnostics:

```sh
WVST_HOST_WORKER=target/debug/wvst-host-worker \
  target/debug/wvst-bridge-server diagnose
```

`diagnose` inspects configuration and worker discovery. It does not process real plugin audio and can run without a token. See [Troubleshooting](/docs/troubleshooting) for recovery steps.

## Next steps

- [Demo guide](/docs/demo-guide): player, rack, parameters and metrics.
- [Web integration](/docs/web-integration): control connection, full audio path and resource cleanup.
- [Configuration and deployment](/docs/configuration): tokens, origins, headers and environment variables.
- [Development](/docs/development): checks, browser regression and real-plugin evidence.
