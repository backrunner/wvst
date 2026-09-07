<script lang="ts">
  import Check from 'phosphor-svelte/lib/Check';
  import ArrowRight from 'phosphor-svelte/lib/ArrowRight';
  import PlugsConnected from 'phosphor-svelte/lib/PlugsConnected';
  import BridgeSetup from '../BridgeSetup.svelte';
  import type { DemoCopy, DemoStatus, Prerequisites } from './types';
  export let t: DemoCopy;
  export let locale: 'en' | 'zh';
  export let connected: boolean;
  export let busy: boolean;
  export let status: DemoStatus;
  export let prerequisites: Prerequisites;
  export let endpoint: string;
  export let token: string;
  export let connectionIssue = '';
  export let onConnect: () => void;
  export let onDisconnect: () => void;
</script>

<section id="studio-connect" class="studio-connection" class:is-connected={connected} aria-label={t.bridgePanel}>
  <div class="connection-overview">
    <span class="connection-icon"><PlugsConnected size={24} /></span>
    <div class="connection-copy"><h2>{connected ? t.connected : t.offlineTitle}</h2><p>{connected ? t.localNote : t.offlineBody}</p></div>
    <button class="studio-button" class:primary={!connected} type="button" onclick={connected ? onDisconnect : onConnect} disabled={busy || status === 'blocked'}>
      {connected ? t.disconnect : status === 'connecting' ? t.connecting : t.connect}
      {#if !connected}<ArrowRight size={16} />{/if}
    </button>
  </div>
  <details class="connection-help" open={!connected}>
    <summary>{t.connectionHelp}</summary>
    {#if !connected}<BridgeSetup {locale} />{/if}
    <div class="connection-options">
      <details class="studio-advanced">
        <summary>{t.advanced}</summary>
        <div class="wvst-connection-fields">
          <label><span>{t.endpoint}</span><input type="url" bind:value={endpoint} spellcheck="false" disabled={connected || busy} /></label>
          <label><span>{t.token}</span><input type="password" bind:value={token} placeholder={t.tokenHint} autocomplete="off" disabled={connected || busy} /></label>
        </div>
        {#if connectionIssue}<p class="connection-detail">{connectionIssue}</p>{/if}
      </details>
      <details class="studio-advanced" open={status === 'blocked'}>
        <summary>{t.diagnostics}</summary>
        <div class="wvst-readiness">
          {#each [[t.secureContext, prerequisites.secureContext], [t.isolation, prerequisites.crossOriginIsolated], [t.sharedBuffer, prerequisites.sharedArrayBuffer]] as item}
            <span class:ready={item[1]}>{#if item[1]}<Check size={14} />{:else}○{/if}{item[0]}</span>
          {/each}
        </div>
        {#if status === 'blocked'}<a href={locale === 'zh' ? '/docs/zh/troubleshooting' : '/docs/troubleshooting'}>{t.guideLink} ↗</a>{/if}
      </details>
    </div>
  </details>
</section>
