# WVST Docs

SvelteKit documentation site for WVST, generated from the local `../svedocs` framework.

```sh
npm run docs:dev
npm run docs:check
npm run docs:build -- --no-og
```

The live rack demo uses real WVST bridge processing. Download a platform package from the [GitHub Releases](https://github.com/backrunner/wvst/releases) page, or build both local binaries before connecting:

```sh
cargo build --release -p wvst-bridge-server -p wvst-host-worker
WVST_HOST_WORKER=target/release/wvst-host-worker \
  target/release/wvst-bridge-server serve
```
