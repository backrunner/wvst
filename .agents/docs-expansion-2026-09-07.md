# Documentation expansion — 2026-09-07

## Scope

Expanded the root English README, added README.zh-CN.md, and maintained nine
public guide pairs (18 articles, 22 site pages including landing/Studio).

- Reworked getting started with prerequisites, both native binaries, required
  CLI token, explicit worker path, expected output and Studio checkpoints.
- Added Web integration with a complete Vite stereo-effect helper, independent
  audio handshake, per-context processor registration and partial-failure /
  idempotent cleanup.
- Added configuration/deployment and development/evidence guides in both locales.
- Expanded Studio operations, API contracts, metric sources and troubleshooting.
- Updated overview reading paths, architecture boundaries and docs/README.md
  authoring, theme, routing, deployment and validation instructions.
- Root READMEs use the independent WVST logo with light/dark variants.

## Corrections confirmed against implementation

- BridgeConfig::from_env requires a nonempty WVST_TOKEN for the standalone CLI;
  token-free embedded development configuration is a different path.
- WVST_ALLOWED_ORIGINS adds exact origins; disable automatic loopback allowance
  explicitly when configuring an exclusive browser allowlist.
- The audio Worker socket needs its own bridge.hello. createHelloRequest does
  not retain the original token, so the caller must provide it again.
- Ordinary RPC results do not all repeat protocol/Bridge versions.
- Browser queue metrics, plugin latency and measured end-to-end latency are
  separate; capacityQuanta is not a fixed latency setting.
- Release binaries are not currently published; @wvst/web is a private workspace
  package. Platform abstractions do not establish third-party compatibility.

The in-product BridgeSetup command and token hint now match the guides. The
smaller effect/instrument examples also lacked the second handshake: fixed
packages/wvst-web-examples/src/shared/wvst.ts, closing control/terminating the
worker on setup failure and retaining the Worker for normal teardown.

## Validation

- Svedocs check: 22 pages, 182 search records, zero errors; existing missing
  site.url warning remains because no production domain is configured.
- Svelte check: zero errors / warnings.
- Production docs build: Cloudflare adapter succeeded; 22 OG SVGs generated.
- Complete Web integration helper and API fragments compiled against actual
  SDK declarations with strict TypeScript and Vite client types.
- Web examples type-check and build passed. An initial concurrent build raced
  the root docs script's SDK clean; sequential build passed after SDK completion.
- Local Markdown links and matching bilingual page order checked.
- Production browser checks: all 18 article routes, local heading anchors,
  cross-origin isolation, 390px mobile layout, 20px mobile heading gap, zero
  uncaught page errors.
- Documentation helper browser run against the local protocol fixture: both
  token-bearing handshakes, 24 real binary audio frames, repeated dispose and
  failed-start instance destruction passed.
- Developer example helper: two token-bearing handshakes and teardown passed
  against the same browser fixture. This is protocol evidence, not a new real
  third-party VST3 compatibility or sustained-audio result.
- git diff --check passed.

Temporary validation scripts/logs are in /tmp/wvst-doc-*,
/tmp/wvst-docs-content-*, and /tmp/wvst-docs-examples-build.log.
