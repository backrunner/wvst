<script lang="ts">
  import { WVST_VERSION } from '@wvst/web';
  import { resolveLocalizedHref } from 'svedocs/theme/headless';
  import type { SvedocsBrandProps } from 'svedocs/theme';
  export let context: SvedocsBrandProps['context'];
  $: href = resolveLocalizedHref(context.config.theme.brand.href, context);
</script>

<a class="sd-brand wvst-brand" {href} aria-label="WVST" data-theme-component="brand">
  <img class="wvst-logo-light" src="/brand/wvst-logo.svg" alt="" width="252" height="70" draggable="false" />
  <img class="wvst-logo-dark" src="/brand/wvst-logo-dark.svg" alt="" width="252" height="70" draggable="false" />
  <span class="wvst-version">v{WVST_VERSION}</span>
</a>

<style>
  .wvst-brand { display: inline-flex; align-items: center; min-height: 44px; gap: 10px; }
  .wvst-version { font-size: 10px; color: var(--sd-text-muted); font-family: monospace; white-space: nowrap; }
  .wvst-brand img { width: 112px; height: auto; border-radius: 0; }
  .wvst-logo-dark { display: none; }
  :global(:root[data-theme="dark"]) .wvst-logo-light { display: none; }
  :global(:root[data-theme="dark"]) .wvst-logo-dark { display: block; }
  @media (max-width: 680px) { .wvst-brand img { width: 100px; } }
</style>
