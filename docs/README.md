# WVST Docs

SvelteKit documentation site for WVST, built with the published Svedocs packages. It includes English and Chinese guides plus a live rack demo backed by the real WVST Web SDK.

From the repository root:

```sh
npm run docs:dev
npm run docs:check
npm run docs:build -- --no-og
```

The site requires Node.js 22. Workspace installation is reproducible with `npm ci`; no adjacent Svedocs checkout is required.

The live rack demo uses real WVST bridge processing. Download a platform package from the [GitHub Releases](https://github.com/backrunner/wvst/releases) page, or build both local binaries before connecting:

```sh
cargo build --release -p wvst-bridge-server -p wvst-host-worker
WVST_HOST_WORKER=target/release/wvst-host-worker \
  target/release/wvst-bridge-server serve
```

The Bridge listens on `ws://127.0.0.1:35876` by default. Local development and production responses include the COOP/COEP headers required by `SharedArrayBuffer`.
