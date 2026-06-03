---
title: Architecture
description: How WVST splits browser audio, control messages, bridge routing, and VST host isolation.
order: 3
---

# Architecture

WVST keeps realtime browser audio and native plugin hosting separated by explicit process and thread boundaries.

```mermaid
flowchart LR
  Player["Browser player"] --> Worklet["AudioWorklet rack slot"]
  Worklet <-->|SAB ring buffers| Worker["DedicatedWorker audio pump"]
  Worker <-->|binary audio frames| Bridge["WVST Bridge Server"]
  Bridge <-->|IPC| Host["Host worker process"]
  Host --> VST["VST3 effect instance"]
```

## Boundaries

- The Web UI owns user interaction, local file playback, rack ordering, bypass state, and generic control surfaces.
- The AudioWorklet only reads/writes bounded audio buffers and never performs network, filesystem, JSON, logging, or async work in `process()`.
- The DedicatedWorker owns WebSocket transport and converts SAB quanta to WVST binary audio frames.
- The Bridge Server handles control requests, stream routing, metrics, origin/token checks, and host worker lifecycle.
- Each VST instance runs in an isolated host worker process so plugin failure does not crash the bridge or browser.

## Rack model

The first demo treats each rack slot as an independent WVST instance. WebAudio serially connects slot worklet nodes, while WVST keeps instance state and stream IDs independent.
