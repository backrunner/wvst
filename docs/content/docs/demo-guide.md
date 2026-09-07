---
title: Live Studio Guide
description: Choose audio, build an effect chain, and understand playback, parameters, bypass, metrics and reconnect behavior.
order: 4
---

# Live Studio Guide

[Open Studio](/demo) to process browser audio through local VST3 effects. The guide follows “connect the Bridge → choose audio → build the effect chain.” You can preview original audio before adding plugins.

For first-time setup, build the Bridge and host worker using [Getting started](/docs/getting-started). You can also use a published portable preview following [Versions and releases](/docs/releases).

## Connect the local Bridge

Start the Bridge with `WVST_TOKEN`, expand advanced connection settings, and enter the same token with endpoint `ws://127.0.0.1:35876`. Keep the Bridge running and connect.

The page tries the default connection once on load, without a preset token. If the first attempt fails authorization, enter the token and retry. For a custom port, change both `WVST_BIND_ADDR` and the page endpoint.

A successful connection means both control and audio-worker sockets are authorized. Plugins come from local discovery. Rescan an empty list and inspect failure paths/reasons. A bundle can expose multiple classes; start with a known stereo input/output effect.

## Choose and play audio

| Action | Behavior |
| --- | --- |
| Try a synth loop | Generate eight seconds of synth audio locally, without downloading a sample. |
| Choose or drop a file | Use browser-supported audio. An accepted extension does not establish decoder support. |
| Play / pause | Play through browser WebAudio; initial playback requires user interaction. |
| Seek / skip | Move through the audio; the waveform represents actual file content. |
| Loop | Repeat the current audio to compare effect settings. |
| Volume / mute | Change output gain after the effects, without changing plugin parameters. |

Files play through local object URLs and are not uploaded to the docs server. With effects enabled, audio samples travel through the local Bridge to plugins. Files over 50 MB skip additional waveform decoding to reduce memory use while playback is still attempted. An empty waveform can also reflect decoder support rather than a transport failure.

Start with WAV or the built-in loop, confirm original playback, then add an effect.

## Build and compare an effect chain

1. Choose and mount a plugin. Every mount creates an independent instance; multiple slots can use the same plugin.
2. Audio flows from top to bottom. Move effects with the order controls to compare different processing orders.
3. Bypass an effect to retain its settings while removing it from the signal path. An empty or fully bypassed rack plays original audio.
4. Removal stops the stream, stops processing and destroys the instance. Remounting creates a new instance.

Distortion before reverb, for example, sounds different from reverb before distortion. The signal path shows enabled effects in order. Bypass does not mean the plugin process has stopped or its CPU resources have been released.

Studio uses 128-frame blocks, stereo input/output, four-quanta buffer capacity and the actual AudioContext sample rate. It does not provide sidechain/multi-bus editing or a MIDI keyboard; use the SDK and repository examples for instruments and device input.

## Parameter controls

The panel shows at most the first eight parameters that are neither hidden nor read-only. An empty panel can mean no exposed parameters, a failed query or no parameters matching those filters; audio processing can still work.

Controls write normalized values and try to display plugin-provided text. The display is refreshed after an edit completes. This is a generic parameter panel, without the plugin's native editor, full preset management or proprietary UI features.

## Understand processing details

| Display | Interpretation |
| --- | --- |
| Pending input/output quanta | Current queue depth, not latency in milliseconds. |
| Underflow | Output was unavailable when the worklet needed it; watch for continued growth. |
| Overflow | Input/queue capacity pressure; correlate with drops and Bridge metrics. |
| Plugin latency | Plugin-reported samples, excluding transport, scheduling and browser buffering. |
| Stereo meters | Actual left/right levels at the final output. |

A counter changing during startup or mounting does not establish long-term instability. Watch deltas during playback. For persistent silence, rapidly increasing counters or glitches, reduce the rack to one effect and follow [Troubleshooting](/docs/troubleshooting).

## Disconnect, reconnect and session limits

Explicit disconnect pauses playback, clears the rack and releases instances. Reconnect and remount effects to continue. The current page's file can be played again, reusing its media source. Leaving the page releases the audio graph and object URL.

Refreshing does not save the rack, file selection or plugin state. Studio has no render export, recording or project-save feature. For persistence in your own app, use the [state snapshot APIs](/docs/api-reference).

## Common workflow questions

| Problem | Next step |
| --- | --- |
| Connection keeps failing | Check that the Bridge runs on the same computer, the token matches and browser local-network access is allowed. |
| Plugin list is empty | Install the VST3 version and rescan; macOS AU plugins are not VST3 scan results. |
| Mount fails | Try a known compatible stereo effect; inspect architecture, class ID and worker diagnostics. |
| Playback is silent | Unmute, check volume/output device, and bypass all effects to verify the original source. |
| Mobile page opens but has no plugins | Responsive layout does not provide a desktop Bridge/VST3 runtime on a phone. |

For your own product, follow [Web integration](/docs/web-integration) for connections, nodes and resource ownership.
