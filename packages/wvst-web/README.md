# @wvst/web

WebAudio SDK for isolated local VST3 processing through the WVST Rust Bridge.
The SDK and native runtime follow the same product version; `WVST_VERSION`
exports the version used by the SDK's default handshake.

## Install a preview tarball

Download `wvst-web-VERSION.tgz` from a WVST GitHub Release, verify it against
`SHA256SUMS`, then install the downloaded file in your application:

```sh
npm install ./wvst-web-VERSION.tgz
```

Replace VERSION with the selected preview version. The package remains private
in the development workspace; releases distribute tarballs, not npm registry
publications. Inside this repository, use `npm ci` and `npm run build:web` instead.

## Connect

```ts
import { WVSTClient, WVST_VERSION } from '@wvst/web';

const client = await WVSTClient.connect({
  endpoint: 'ws://127.0.0.1:35876',
  token: tokenFromUser,
  requireLowLatency: true
});
console.log({ sdk: WVST_VERSION, bridge: client.hello.bridgeVersion });
```

`tokenFromUser` is the token configured for the local Bridge. The browser page
needs a secure context, SharedArrayBuffer, AudioWorklet and COOP/COEP headers.
Control connection alone does not process audio: the transport Worker needs its
own handshake and a started stream. Follow the full integration guide for nodes,
MIDI, parameters, snapshots and cleanup.

- Documentation: https://wvst-docs.pages.dev/docs/web-integration
- 中文文档：https://wvst-docs.pages.dev/docs/zh/web-integration
- Versions and releases: https://github.com/backrunner/wvst/releases
- Source: https://github.com/backrunner/wvst/tree/main/packages/wvst-web

Native plugin editors and zero-latency processing are not promised. Third-party
plugins are installed separately and keep their own licenses.

Licensed under MIT OR Apache-2.0; see the included license files.
