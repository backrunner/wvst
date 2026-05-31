---
name: wvst-engineering
description: Use for WVST project work involving Rust, WebAudio, WASM, local bridge servers, VST3 hosting, realtime audio, plugin isolation, module design, roadmap updates, or engineering standards. Apply before writing or reviewing WVST code or docs.
---

# WVST Engineering

## First steps

- Read `.agents/README.md` for the current document map.
- For product behavior, read `.agents/requirements.md`.
- For architecture or module placement, read `.agents/module-design.md`.
- For implementation rules, read `.agents/development-standards.md`.
- For sequencing work, read `.agents/roadmap.md`.

## Hard constraints

- Rust-first for native code, protocol, VST host, bridge, scanner, process supervision, and test tooling.
- TypeScript is allowed for the Web SDK and Rollup packaging; high-frequency shared logic should prefer Rust -> WASM.
- Bridge Server must not load third-party VST plugins in-process.
- VST plugins run in host worker processes; crash and ANR handling are product requirements, not cleanup tasks.
- WebAudio realtime code must never block waiting for native processing.
- AudioWorklet hot paths must avoid allocation, JSON, logging, network, filesystem, locks, and Promise/await.
- Support VST effects and instrument plugins; MIDI/note events need sample offsets.
- Same plugin must support N independent instances with independent state.
- macOS is first in the roadmap, but platform abstractions must leave room for Windows and Linux.

## Implementation posture

- Keep files under 400 lines when practical and split before 600 lines unless there is a documented reason.
- Put unsafe Rust only in explicit FFI/ABI boundary modules with safety notes.
- Use binary protocol for audio frames and structured versioned schema for control messages.
- Expose latency, jitter, underflow, overflow, worker restart, and plugin crash metrics.
- Prefer explicit state machines for VST lifecycle, worker lifecycle, pairing, and stream state.
- Add tests at protocol boundaries and realtime-adjacent ring-buffer logic before broad feature work.

## Web integration reminders

- `SharedArrayBuffer` requires secure context and cross-origin isolation.
- DedicatedWorker owns transport; AudioWorklet owns realtime buffer exchange.
- If SAB or low-latency prerequisites fail, report the failure clearly instead of silently degrading.
- Generic parameter UI is the default; native VST UI embedding in browser is not a core promise.

## Commit rules

Use this identity for commits:

```text
BackRunner <dev@backrunner.top>
```

Use this message format:

```text
type(scope): subject
```

Examples: `docs(agents): add initial feasibility and architecture docs`, `feat(protocol): add audio frame header`, `perf(ringbuf): reduce copies`.

