---
title: Overview
description: WVST turns local VST3 effects into WebAudio processing nodes through an isolated bridge.
order: 1
---

# Overview

WVST is an experimental Rust-first bridge between WebAudio and local VST3 plugins. A web app connects to a localhost Bridge Server, scans available plugins, creates isolated plugin instances, and routes audio through AudioWorklet + SharedArrayBuffer.

The browser never blocks the realtime audio callback waiting for native processing. Audio moves through bounded buffers, a DedicatedWorker transport, and a host worker process that owns the third-party VST instance.

## Current shape

- Web SDK: `WVSTClient`, plugin scan/list APIs, instance lifecycle APIs, loopback AudioWorklet helpers, and a bridge worker audio pump.
- Bridge Server: localhost WebSocket control and binary audio routing, origin/token checks, metrics, stream lifecycle, and worker supervision.
- Host Worker: isolated process for plugin probing, instance lifecycle, and the first VST3 processing path.

## Read next

- [Getting started](/docs/getting-started)
- [Architecture](/docs/architecture)
- [Live demo](/docs/live-demo)
- [Troubleshooting](/docs/troubleshooting)
