---
title: Development
description: Workspace checks, Studio browser regression, real-plugin evidence, and reproducible issue reports.
order: 9
---

# Development

Different checks answer different questions: type and unit tests check implementation contracts, browser fixtures exercise UI/protocol behavior, and real VST3 runs validate specific machine/plugin combinations. Match release claims to the evidence collected.

## Prepare the workspace

```sh
npm ci
rustup component add rustfmt clippy
rustup target add wasm32-unknown-unknown
```

Use stable Rust and Node.js 22. Prioritize real macOS plugins matching the worker architecture for compatibility work. Read `.agents/implementation-gap-analysis.md` and `.agents/development-standards.md` to distinguish implemented behavior from roadmap goals.

## Select checks for the change

Run from the repository root:

| Command | Scope |
| --- | --- |
| `npm run check` | Rust tests, WASM target check, SDK types/unit tests and example types. |
| `cargo fmt --all -- --check` | Rust formatting. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Rust lint with warnings as errors. |
| `npm run check:web` | SDK TypeScript. |
| `npm run test:web` | SDK Vitest tests. |
| `npm run build:web:examples` | SDK and effect/instrument example bundles. |
| `npm run docs:check` | Svedocs content checks and Svelte types. |
| `npm run docs:build` | SDK and production docs, including OG SVG generation. |
| `npm run docs:build -- --no-og` | Docs build without OG generation. |

`npm run check` does not build the docs or run Studio browser regression. Documentation-only changes normally need docs check/build and page inspection; audio protocol, resource lifecycle and realtime changes need corresponding tests and evidence.

## Studio browser regression

Build first, then start production preview. Avoid running the dev server while a production build writes the same generated SvelteKit files.

Terminal A:

```sh
npm run docs:build
npx playwright install chromium
npm --workspace @wvst/docs run preview
```

Terminal B:

```sh
npm --workspace @wvst/docs run smoke:studio
```

The default target is `http://127.0.0.1:4173`. To override it or save screenshots:

```sh
mkdir -p /tmp/wvst-studio-shots
WVST_STUDIO_URL=http://127.0.0.1:4173 \
WVST_STUDIO_SCREENSHOTS=/tmp/wvst-studio-shots \
  npm --workspace @wvst/docs run smoke:studio
```

A local protocol fixture covers control/audio handshakes, binary audio round trips, file and generated sources, waveform, empty scans, failed-mount cleanup, parameters, reorder, bypass, removal, reconnect, both locales and mobile layout. It does not load third-party VST3 code or establish audio quality or long-run stability.

## Real-plugin and sustained-audio evidence

Prepare locally installed VST3 fixtures you are entitled to use. Record vendor, name, version, type, CPU architecture, channel layout, sample rate and block size. Keep proprietary plugin binaries out of the repository.

Available tooling:

| Tool or entry | Purpose |
| --- | --- |
| `wvst-runtime-matrix` | Run isolated plugin probes from a manifest and evaluate case expectations. |
| `npm run smoke:bridge:web` | SDK-to-local-Bridge/plugin smoke; read the script's fixture environment requirements first. |
| `npm run smoke:browser:web` | Browser WebAudio validation with the configured test environment. |
| `npm run evidence:web:bridge-long` | Configure a 30-minute Bridge smoke window. |
| `npm run evidence:web:browser-long` | Configure a 30-minute browser smoke window. |
| `wvst-latency-snapshot` | Normalize per-block observations into a latency snapshot. |
| `wvst-stability-budget` | Evaluate latency, drops, errors and optional browser/Bridge metric budgets. |
| `wvst-package-evidence` | Validate package manifests, reports and runtime file roles. |

Copy and edit `.agents/runtime-probe-matrix.example.json` with real local paths and appropriate expectations, then run:

```sh
cargo build -p wvst-host-worker
cargo run -p wvst-testkit --bin wvst-runtime-matrix -- \
  --worker target/debug/wvst-host-worker \
  --manifest /absolute/path/to/local-matrix.json
```

The manifest's example paths are not bundled plugins. Configure a self-hosted runner using `.github/workflows/evidence.yml`; ordinary cross-platform compilation in CI does not establish third-party compatibility on every platform.

## Report a reproducible problem

Include enough information to locate the failing layer:

- Commit, OS version, CPU architecture and browser version.
- Minimal steps from a clean start, expected behavior and actual result.
- Plugin vendor, name, version, class ID, effect/instrument type and channels.
- `AudioContext.sampleRate`, block frames, buffer capacity and effect-chain order.
- `diagnose` output, relevant `bridge.events()` or `instance.runtimeSnapshot()`, and counter deltas over a stated interval.
- Whether bypass, a single instance, reconnect or another plugin changes the failure.

Remove tokens and unnecessary personal paths before sharing. CPU, end-to-end latency and stability claims should identify measurement method, duration and plugin combination.

## Contribution conventions

Prefer Rust for native implementation; keep VST3 ABI and unsafe code at explicit boundaries. The Bridge must not load third-party plugins. AudioWorklet must not wait for native processing or introduce JSON, networking, logging or blocking work. Add tests for protocol/lifecycle changes and update both documentation locales.

See repository `docs/README.md` and `docs/brand.md` for theme, content routing and logo maintenance.
