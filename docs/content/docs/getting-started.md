---
title: Getting Started
description: Prepare the local bridge, browser headers, and the Web SDK path used by WVST.
order: 2
---

# Getting Started

WVST needs both a browser app and a local bridge. The browser side must run in a secure, cross-origin isolated context because the low-latency path uses `SharedArrayBuffer`.

## Build the web SDK

```sh
npm run build:web
```

The docs site imports `@wvst/web` from the workspace, so build the SDK before running the demo.

## Run the bridge

```sh
cargo run -p wvst-bridge-server
```

The default endpoint is `ws://127.0.0.1:35876`. The bridge listens on loopback and accepts loopback browser origins by default.

## Run the docs

```sh
npm run docs:dev
```

The docs dev server sends `Cross-Origin-Opener-Policy` and `Cross-Origin-Embedder-Policy` headers so WVST low-latency mode can allocate `SharedArrayBuffer`.

## Use the demo

Open [Live Demo](/docs/live-demo), connect to the bridge, select a local audio file, scan plugins, and mount one or more 2-in/2-out effect VSTs in the rack.
