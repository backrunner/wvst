---
title: WVST
description: Run local VST effects from WebAudio through an isolated Rust bridge.
layout: home
---

# WVST

WVST connects browser audio graphs to local VST3 plugins without loading third-party plugins inside the browser or the bridge server. The web app owns the interface and routing; the local bridge owns discovery, worker supervision, and native processing.

Start with the [documentation](/docs), or open the [live rack demo](/docs/live-demo) when the local bridge is running.
