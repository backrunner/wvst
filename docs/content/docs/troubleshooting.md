---
title: Troubleshooting
description: Common setup errors for the WVST docs demo and low-latency browser path.
order: 5
---

# Troubleshooting

## SharedArrayBuffer is unavailable

Use `npm run docs:dev` or another server that sends both headers:

```txt
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

WVST low-latency mode fails clearly when these headers are missing.

## Bridge connection fails

Start the bridge and keep the endpoint at the default unless you changed `WVST_BIND_ADDR`:

```sh
cargo run -p wvst-bridge-server
```

If you set `WVST_TOKEN`, enter the same token in the demo before connecting.

## No plugins appear

Run a rescan from the demo. On macOS the scanner checks the standard VST3 locations, including `/Library/Audio/Plug-Ins/VST3` and the user Library path.

## A mounted slot is silent

The first demo expects stereo effect plugins. Instrument plugins, unsupported bus arrangements, stopped streams, and crashed workers will show a slot error or underflow/overflow counters.
