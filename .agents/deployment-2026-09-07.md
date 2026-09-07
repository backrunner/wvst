# Cloudflare Pages deployment — 2026-09-07

The public documentation uses the direct-upload Cloudflare Pages project
`wvst-docs`, with production branch `main` and URL
`https://wvst-docs.pages.dev`.

`docs/wrangler.jsonc` declares `.svelte-kit/cloudflare` as the Pages output and
pins a compatibility date. Wrangler 4.129.0 is a direct docs dev dependency.
`npm run docs:deploy` checks/builds the site and uploads it. Non-main branches
produce preview deployments; a Git push runs CI but does not automatically deploy.

The site URL, English/Chinese deployment guides and READMEs now use the Pages
address. Keep this URL aligned when a custom domain is adopted. Preserve both
SvelteKit hook headers and the Pages `_headers` file: prerendered HTML needs
asset-level COOP/COEP to keep the Studio cross-origin isolated.

## CI corrections discovered after the first push

- Docs checks on clean CI lacked `.svelte-kit/tsconfig.json`. The check command
  now runs `svelte-kit sync` before `svelte-check`. Verified locally after moving
  the entire generated `.svelte-kit` directory out of the workspace.
- Windows rejected an unconditional mutable DirBuilder under `-D warnings`.
  The builder is now mutable only in the Unix permission-configuration block;
  Unix mode 0700 and directory creation behavior are preserved.

Local validation: 22 pages / 182 search records, no content or Svelte errors or
warnings; production Cloudflare build with 22 OG SVGs; Rust formatting and 17
shared-memory tests passed. Credentials remain in local Wrangler/GitHub auth
storage and are not committed. Deployment status and exact artifact URLs are
available from `wrangler pages deployment list` and the Cloudflare dashboard.
