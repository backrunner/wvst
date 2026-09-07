# WVST Docs

SvelteKit documentation site for WVST, built with the published Svedocs packages. It includes English and Chinese guides plus a live rack demo backed by the real WVST Web SDK.

From the repository root:

```sh
npm run docs:dev
npm run docs:check
npm run docs:build
```

The site requires Node.js 22. Workspace installation is reproducible with `npm ci`; no adjacent Svedocs checkout is required.

The live rack demo uses real WVST bridge processing. Developer previews are managed through [GitHub Releases](https://github.com/backrunner/wvst/releases); only published releases expose downloads. For source development, build both binaries before connecting:

```sh
cargo build --release -p wvst-bridge-server -p wvst-host-worker
WVST_TOKEN=local-dev-token \
WVST_HOST_WORKER=target/release/wvst-host-worker \
  target/release/wvst-bridge-server serve
```

The Bridge CLI requires a nonempty token. Enter `local-dev-token` in Studio’s advanced connection settings after running the example command. The initial automatic connection has no token and may report an authorization error. The Bridge listens on `ws://127.0.0.1:35876` by default. Vite dev/preview headers, the SvelteKit server hook, and `_headers` for Cloudflare assets provide the COOP/COEP headers required by `SharedArrayBuffer`. Other static hosts must configure these two headers themselves; prerendered pages do not run server hooks.

The documentation runtime and CLI are pinned together to Svedocs `0.2.1`.

The WVST theme is registered through the public `theme.components.Home` and `theme.components.Brand` extensions
in `vite.config.ts`. `src/lib/theme/WVSTHome.svelte` owns the bilingual landing
page, and `SignalConsole.svelte` provides the interactive architecture diagram.
Its waveform is illustrative; it does not represent a live connection or latency
measurement. The console supports keyboard navigation and reduced motion.

`wvst-theme.css` defines the warm paper / graphite palette shared by documentation
and navigation; `wvst-home.css` controls the responsive landing layout. Keep the
rack demo's styles in `wvst-rack-demo.css` and preserve the Svedocs Root context
when replacing components so search, locale navigation and theme switching work.

Both `npm run docs:build` and `npm run docs:build -- --no-og` are supported. The
OG endpoint uses automatic prerendering because the CLI can generate the same URLs
as static assets before SvelteKit runs. The production `site.url` is
`https://wvst-docs.pages.dev`, configured in `svedocs.config.ts`.

## Live Studio

The `/demo` and `/zh/demo` pages use the `studio` content layout rather than an
article shell. The three-step guide links to Bridge setup, audio selection and
the effect chain. Original audio can be previewed without a Bridge connection;
VST processing uses an authenticated control socket and a separately
authenticated audio-worker socket.

The built-in eight-second synth loop is generated locally. Waveform decoding
runs outside the realtime path and is skipped for files over 50 MB. Uploaded
files are played through the browser media element. `rack-demo/audio-graph.ts`
owns the media source, stereo output, meters and worklet-node setup; the source
is reused on reconnect and disposed when leaving the studio.

The studio CSS entry imports `studio-shell.css`, `studio-player.css` and
`studio-rack.css`. Labels and guidance are shared through `rack-demo/copy.ts`.

To run the browser regression against a running production preview:

```sh
npm run docs:build
npx playwright install chromium
npm --workspace @wvst/docs run preview
# In another terminal:
npm --workspace @wvst/docs run smoke:studio
```

Set `WVST_STUDIO_URL` for a different preview URL, and optionally
`WVST_STUDIO_SCREENSHOTS` to an existing directory for screenshots. The smoke
uses a local protocol fixture to check playback, waveform generation, both
socket handshakes, scan results, failure cleanup, parameter edits, reordering,
bypass, removal, reconnect, mobile layout and Chinese copy. It does not replace
real VST3 compatibility or audio stability testing.

The standalone logo files and their usage are documented in [brand.md](brand.md).

## Content and routes

| Source | Route / responsibility |
| --- | --- |
| `content/docs/index.md` | `/docs`, English overview |
| `content/docs/zh/index.md` | `/docs/zh`, Chinese overview |
| `content/docs/<slug>.md` | `/docs/<slug>`, English guide |
| `content/docs/zh/<slug>.md` | `/docs/zh/<slug>`, matching Chinese guide |
| `content/pages/index.md`, `content/pages/zh/index.md` | `/`, `/zh`, landing content |
| `content/pages/demo.svx`, `content/pages/zh/demo.svx` | `/demo`, `/zh/demo`, Studio pages |
| `svedocs.config.ts` | Site identity, navigation, locales, search and SEO |
| `vite.config.ts` | Public Svedocs component/layout registrations and isolation headers |

The public guides cover overview, setup, architecture, Studio, Web integration,
API reference, configuration/deployment, troubleshooting and development. Keep
matching locale slugs and ordering together. Frontmatter uses `title`,
`description` and numeric `order`:

```yaml
---
title: Guide title
description: The reader task this page solves.
order: 10
---
```

A new page in `content/docs` is discovered by Svedocs; add its translated peer,
link it from the overview and root README, and run the content checks. Use
`/docs/zh/...` in Chinese article links and `/zh/demo` for Chinese Studio; those
are different route conventions. Keep filenames stable when translating titles.

## Authoring conventions

- Explain the reader's outcome and prerequisites before commands. Run shell
  examples from the repository root unless another directory is stated.
- Use implemented SDK names and options. `.agents` designs include proposals;
  verify public instructions against code, not only roadmap prose.
- Separate self-contained examples from API fragments, naming caller-owned
  objects. Include errors, empty scan results, authorization and cleanup when
  showing a full lifecycle.
- Maintain English and Chinese together. Keep code identifiers and environment
  variables unchanged while translating explanations naturally.
- Distinguish Studio fixture coverage from real-plugin compatibility and
  measured end-to-end latency from plugin-reported sample delay.
- Use normal Markdown heading hierarchy. Let `.sd-prose` styles provide divider
  and heading gaps; do not insert repeated blank elements or manual separators
  to force spacing. An intentional `hr` before an H2 is handled by the theme.

## Theme and brand map

| File | Responsibility |
| --- | --- |
| `src/lib/theme/WVSTBrand.svelte` | Localized brand link and light/dark logo selection |
| `src/lib/theme/WVSTHome.svelte` | Bilingual landing composition |
| `src/lib/theme/SignalConsole.svelte` | Illustrative interactive architecture console |
| `src/lib/theme/WVSTStudioLayout.svelte` | Standalone Studio shell |
| `src/lib/styles/wvst-theme.css` | Shared palette, navigation and article typography/spacing |
| `src/lib/styles/wvst-home.css` | Landing layout |
| `src/lib/styles/wvst-rack-demo.css` | Studio stylesheet entry importing the three Studio style files |
| `static/brand` | Independent logo, mark, dark and monochrome variants, brand sheet |

Preserve Svedocs Root context when customizing layouts. Check search, locale
switching, theme switching, keyboard focus and reduced motion after theme edits.
Avoid exposing runtime implementation details in the normal Studio workflow;
put deeper explanations in the guides and expandable processing details.

## Build and deployment checklist

1. Run `npm run docs:check` and `npm run docs:build` from the repository root.
2. Start production preview after the build; avoid a concurrent dev server
   writing SvelteKit generated files.
3. Open both locale overviews and new pages; verify navigation, search, code,
   tables, heading anchors, dark mode and narrow-screen overflow.
4. Verify COOP/COEP on the final page response and JavaScript responses for
   Worker/worklet assets. Prerendered HTML does not execute server hooks.
5. Keep `site.url` aligned with the production domain when moving to a custom
   domain; it currently points to `https://wvst-docs.pages.dev`.

The default build uses the Cloudflare adapter. For another static host run
`npm run build:web` followed by `npm --workspace @wvst/docs run build:static`
and deploy `docs/build`, configuring the required headers on that host. See
[configuration](content/docs/configuration.md) / [配置与部署](content/docs/zh/configuration.md)
for the full local-runtime and web-hosting distinction.

## Production deployment

The Cloudflare Pages project is `wvst-docs`, production branch `main`:

- English: https://wvst-docs.pages.dev/
- Chinese: https://wvst-docs.pages.dev/zh
- Studio: https://wvst-docs.pages.dev/demo

`wrangler.jsonc` declares the Pages output and compatibility date; Wrangler is
pinned in this workspace. After `wrangler login` with an account that can deploy
to the project, run from the repository root:

```sh
npm run docs:deploy
```

This checks and builds the site, then uploads the Cloudflare adapter output.
Wrangler selects the deployment environment from the Git branch: `main` updates
production; other branches produce previews. To deploy an already checked build,
run `npm --workspace @wvst/docs run deploy`. Inspect the reported deployment URL
and the production alias, including COOP/COEP on `/demo` and deep article URLs.

Deployments are currently direct uploads. A Git push runs repository CI and does
not automatically publish the site. No Cloudflare credentials belong in this
repository. The browser still needs the user's local Bridge and its token to
process VST3 audio.

## Product version and release docs

WVST product versions are synchronized from the Rust workspace by
`npm run release:prepare`; the generated SDK `WVST_VERSION` drives navigation
and Studio handshake identity. Svedocs dependency versions and wire/schema
versions are separate. See [Versions and releases](content/docs/releases.md)
and [版本与发布](content/docs/zh/releases.md). The site tracks the current
checkout; Git tags retain older source/doc versions. Deploy the docs explicitly
after publishing a preview so visible version and release instructions agree.
