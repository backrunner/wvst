<script lang="ts">
  import { RootLayout } from 'svedocs/theme';
  import type { SvedocsCustomLayoutProps } from 'svedocs/theme';
  export let page: SvedocsCustomLayoutProps['page'];
  export let pages: SvedocsCustomLayoutProps['pages'] = [];
  export let tree: SvedocsCustomLayoutProps['tree'] = [];
  export let search: SvedocsCustomLayoutProps['search'] = [];
  export let config: SvedocsCustomLayoutProps['config'];
  export let content: SvedocsCustomLayoutProps['content'];
  export let loadSearch: SvedocsCustomLayoutProps['loadSearch'] = undefined;
  export let themeComponents: NonNullable<SvedocsCustomLayoutProps['themeComponents']> = {};
  $: Root = themeComponents.Root ?? RootLayout;
</script>

<svelte:component this={Root} {page} {pages} {tree} {search} {config} {loadSearch} {themeComponents}>
  <main id="content" class="wvst-studio-page">
    {#if content}<svelte:component this={content} />{/if}
  </main>
</svelte:component>
