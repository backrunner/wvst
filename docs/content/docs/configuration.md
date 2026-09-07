---
title: Configuration and Deployment
description: Bridge environment variables, token and origin policy, browser isolation headers, and documentation hosting checks.
order: 7
---

# Configuration and Deployment

WVST has two deployment targets: the browser-facing site, and the Bridge/host worker on the user's computer. Hosting the docs does not install, start or remotely replace the user's local plugin runtime.

## Local authorization

The standalone CLI's `serve` requires a nonempty `WVST_TOKEN`. Each WebSocket must complete `bridge.hello` with that token, including the separate UI and audio-worker sockets. Embedded `BridgeConfig::development()` can use a token-free policy; that is not the CLI default.

This local development configuration allows one exact page origin:

```sh
WVST_TOKEN=local-dev-token \
WVST_ALLOWED_ORIGINS=http://localhost:5173 \
WVST_ALLOW_LOOPBACK_ORIGINS=false \
WVST_HOST_WORKER=target/debug/wvst-host-worker \
  target/debug/wvst-bridge-server serve
```

Open `http://localhost:5173` and enter the same token in Studio. `http://127.0.0.1:5173` is a different origin, and changing the port changes the origin too.

`WVST_ALLOWED_ORIGINS` adds exact allowed values; loopback pages remain automatically allowed by default. Set `WVST_ALLOW_LOOPBACK_ORIGINS=false` as well to disable that automatic rule. Supply the webpage origin (scheme, host, port), without `/demo` or the Bridge's WebSocket URL.

Origin checks constrain browser sources. Native clients without an Origin are not rejected by that policy, so it does not replace token authorization. The CLI uses a configured shared token; it does not provide a complete short-lived pairing-code, user-account or automatic browser enrollment flow. Rotate the configured token by restarting the Bridge and reconnecting clients.

## Bridge environment variables

Configuration is read at startup. Run `diagnose` with the same environment when investigating; variables in another shell do not change an existing process.

| Variable | Default | Meaning |
| --- | --- | --- |
| `WVST_TOKEN` | Unset; required by CLI | Nonempty session token; keep it out of public frontend configuration. |
| `WVST_BIND_ADDR` | `127.0.0.1:35876` | Listen address and port; match the browser endpoint. Keep deployment on loopback. |
| `WVST_HOST_WORKER` | Auto-discovery | Executable path; prefer an absolute path for services or other working directories. |
| `WVST_ALLOWED_ORIGINS` | Empty list | Comma-separated exact additional origins. |
| `WVST_ALLOW_LOOPBACK_ORIGINS` | `true` | `0` or `false` disables automatic allowance of local pages. |
| `WVST_WORKER_AUTO_RESTART` | `true` | `0` or `false` disables automatic worker restart. |
| `WVST_MAX_WORKER_INSTANCES` | `64` | Concurrent worker cap, not a performance promise for 64 realtime plugins. |
| `WVST_WORKER_QUARANTINE_FAILURES` | `3` | Failure threshold for plugin quarantine. |
| `WVST_MAX_CONTROL_MESSAGE_BYTES` | `16777216` (16 MiB) | JSON control message cap, including large state snapshots. |
| `WVST_WORKER_MEMORY_LIMIT_BYTES` | Unset | Worker address-space limit where supported; not an RSS measurement. |
| `WVST_WORKER_CPU_TIME_LIMIT_SECONDS` | Unset | Cumulative CPU time limit where supported, not an individual process-call timeout. |
| `WVST_WORKER_LINUX_CGROUP_PARENT` | Unset | Writable Linux cgroup parent. |
| `WVST_WORKER_LINUX_CGROUP_MEMORY_MAX_BYTES` | Unset | Cgroup memory ceiling; requires cgroup setup. |
| `WVST_WORKER_LINUX_CGROUP_CPU_QUOTA_MICROS` | Unset | Cgroup CPU quota in microseconds. |
| `WVST_WORKER_LINUX_CGROUP_CPU_PERIOD_MICROS` | `100000` | Cgroup CPU quota period in microseconds. |

Inspect events and worker diagnostics before adjusting limits for repeated failures. Larger limits do not fix architecture or bus-layout incompatibility.

## Browser isolation and assets

Serve low-latency pages over HTTPS or browser-trusted localhost with:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

Check the final page response, not only configuration files:

```sh
curl -I http://localhost:4173/demo
```

Verify `isSecureContext`, `crossOriginIsolated` and `typeof SharedArrayBuffer === 'function'` in the console. COEP affects third-party images, fonts, scripts and embeds: use same-origin assets or appropriate CORS/CORP responses. Worker and worklet URLs must return JavaScript, not HTML from an SPA fallback.

Isolation and local WebSocket access are separate checkpoints. A remote HTTPS page accessing `ws://127.0.0.1:35876` is also subject to browser mixed-content rules, local-network permissions and enterprise policies. The Bridge CLI serves `ws://`; changing the URL to `wss://` does not enable TLS. Deployments needing TLS must supply a trusted local TLS terminator. Establish a working localhost setup first, then validate remote hosting in the target browser.

## Build the docs site

From the repository root:

```sh
npm ci
npm run docs:check
npm run docs:build
npm --workspace @wvst/docs run preview
```

The default build uses the Cloudflare adapter. Local preview normally prints `http://localhost:4173`. For static hosting:

```sh
npm run build:web
npm --workspace @wvst/docs run build:static
```

The static adapter's default output is `docs/build`. Do not upload an edge build as if it were a plain static directory; match deployment to the adapter.

| Execution mode | Header provider |
| --- | --- |
| Vite development | `server.headers` in `docs/vite.config.ts`. |
| Local production preview | Preview middleware in the same file. |
| SvelteKit dynamic responses | `docs/src/hooks.server.ts`. |
| Cloudflare assets | Asset rules in `docs/_headers`. |
| Other static hosting | Configure the host or reverse proxy; static pages do not execute server hooks. |

The production site is `https://wvst-docs.pages.dev`. Its `site.url` is configured in `docs/svedocs.config.ts` for canonical, sitemap and OG URLs. Update that value when adopting a custom domain.

The Cloudflare Pages project is `wvst-docs`, with production branch `main`. After authenticating Wrangler, run `npm run docs:deploy` from the repository root to check, build and upload. The project uses direct uploads: Git pushes run CI but do not automatically deploy. Other Git branches produce preview deployments.

## Verify after deployment

1. Open both locales, the homepage and Studio; refresh a deep documentation route.
2. Inspect response headers and ensure Worker, worklet, logo and search assets load.
3. Check browser prerequisites, then connect to a local Bridge with its token.
4. Add the actual webpage origin to the Bridge allowlist and handle any browser local-network permission required by the deployment.
5. Play a local file, mount a known working effect, then check parameters, bypass and metrics.

See repository `docs/README.md` for site maintenance and [Development](/docs/development) for package and real-plugin validation.
