<script lang="ts">
  import ArrowDown from 'phosphor-svelte/lib/ArrowDown';
  import ArrowUp from 'phosphor-svelte/lib/ArrowUp';
  import ArrowsClockwise from 'phosphor-svelte/lib/ArrowsClockwise';
  import Plug from 'phosphor-svelte/lib/Plug';
  import Power from 'phosphor-svelte/lib/Power';
  import Trash from 'phosphor-svelte/lib/Trash';
  import Warning from 'phosphor-svelte/lib/Warning';
  import ParameterControls from './ParameterControls.svelte';
  import type { DemoCopy, PluginChoice, RackSlot } from './types';

  export let t: DemoCopy;
  export let locale: 'en' | 'zh';
  export let pluginChoices: PluginChoice[] = [];
  export let selectedChoiceKey = '';
  export let connected = false;
  export let busy = false;
  export let rack: RackSlot[] = [];
  export let scanFailures: Array<{ path: string; reason: string }> = [];
  export let onMountEffect: () => void;
  export let onScan: () => void;
  export let onMove: (index: number, direction: -1 | 1) => void;
  export let onBypass: (slot: RackSlot) => void;
  export let onRemove: (slot: RackSlot) => void;
  export let onPreviewParameter: (slotId: string, parameterId: number, value: number) => void;
  export let onCommitParameter: (slotId: string, parameterId: number, value: number) => void;

  $: selected = pluginChoices.some((choice) => choice.key === selectedChoiceKey);
</script>

<section id="studio-rack" class="wvst-rack" aria-label={t.rackPanel}>
  <header class="wvst-panel-header">
    <div><span class="panel-index">02 / {t.rackPanel}</span><h2>{t.rackTitle}</h2></div>
    <span class="rack-count">{rack.length.toString().padStart(2, '0')} FX</span>
  </header>
  <p class="rack-intro">{t.rackHint}</p>
  <div class="wvst-rack-toolbar">
    <label><span>{t.selectPlugin} <small>{pluginChoices.length} {t.available}</small></span>
      <select bind:value={selectedChoiceKey} disabled={!connected || !pluginChoices.length || busy}>
        <option value="">{connected ? t.selectPlugin : t.stepBridge}</option>
        {#each pluginChoices as choice}<option value={choice.key}>{choice.label} — {choice.sublabel}</option>{/each}
      </select>
    </label>
    <button class="wvst-icon-button" type="button" title={t.scan} aria-label={t.scan} on:click={onScan} disabled={!connected || busy}><ArrowsClockwise size={19} class={busy ? 'spinning' : ''} /></button>
    <button class="studio-button primary" type="button" on:click={onMountEffect} disabled={!connected || !selected || busy}><Plug size={17} />{t.add}</button>
  </div>
  {#if rack.length === 0}
    <div class="wvst-rack-empty">
      <div class="empty-chain" aria-hidden="true"><span>IN</span><i></i><b><Plug size={30} weight="light" /></b><i></i><span>OUT</span></div>
      <h3>{!connected ? t.rackOffline : !pluginChoices.length ? t.rackNoPlugins : t.rackReady}</h3>
      <p>{!connected ? t.rackOfflineBody : !pluginChoices.length ? t.rackNoPluginsBody : t.rackReadyBody}</p>
      {#if !connected}<a class="studio-inline-link" href="#studio-connect">{t.stepBridge} ↑</a>
      {:else if !pluginChoices.length}<a class="studio-inline-link" href={locale === 'zh' ? '/docs/zh/demo-guide' : '/docs/demo-guide'}>{t.guideLink} ↗</a>{/if}
    </div>
  {:else}
    <ol class="wvst-slot-list">
      {#each rack as slot, index (slot.id)}
        <li class:bypassed={slot.bypassed} class:removing={slot.state === 'removing'}>
          <div class="wvst-slot-topline">
            <span class="wvst-slot-index">{String(index + 1).padStart(2, '0')}</span>
            <div class="wvst-slot-title">
              <strong>{slot.choice.label}</strong>
              <span>{slot.choice.sublabel}</span>
              <small class="slot-state">{slot.state === 'removing' ? t.removing : slot.bypassed ? t.effectBypassed : t.effectActive}</small>
            </div>
            <div class="wvst-slot-actions">
              <button type="button" title={t.up} aria-label={t.up} on:click={() => onMove(index, -1)} disabled={index === 0 || busy || slot.state === 'removing'}><ArrowUp size={17} /></button>
              <button type="button" title={t.down} aria-label={t.down} on:click={() => onMove(index, 1)} disabled={index === rack.length - 1 || busy || slot.state === 'removing'}><ArrowDown size={17} /></button>
              <button class:active={!slot.bypassed} type="button" title={slot.bypassed ? t.enable : t.bypass} aria-label={slot.bypassed ? t.enable : t.bypass} aria-pressed={!slot.bypassed} on:click={() => onBypass(slot)} disabled={busy || slot.state === 'removing'}><Power size={17} weight="bold" /></button>
              <button class="danger" type="button" title={t.remove} aria-label={t.remove} on:click={() => onRemove(slot)} disabled={busy || slot.state === 'removing'}><Trash size={17} /></button>
            </div>
          </div>


          <ParameterControls
            {t}
            slotId={slot.id}
            parameters={slot.parameters}
            disabled={busy || slot.bypassed}
            onPreview={onPreviewParameter}
            onCommit={onCommitParameter}
          />
          <details class="slot-diagnostics"><summary>{t.technical}</summary>
          <div class="wvst-slot-metrics" aria-label={t.meters}>
            <span><small>{t.inputQueue}</small><strong>{slot.metrics.pendingInputQuanta}</strong></span>
            <span><small>{t.outputQueue}</small><strong>{slot.metrics.pendingOutputQuanta}</strong></span>
            <span><small>{t.underflows}</small><strong>{slot.metrics.underflows}</strong></span>
            <span><small>{t.overflows}</small><strong>{slot.metrics.overflows}</strong></span>
            <span><small>{t.pluginLatency}</small><strong>{slot.instance.latencySamples} smp</strong></span>
          </div>

          </details>
        </li>
      {/each}
    </ol>
  {/if}

  {#if scanFailures.length > 0}
    <details class="wvst-failures">
      <summary><Warning size={17} />{t.failures} ({scanFailures.length})</summary>
      {#each scanFailures as failure}
        <p><strong>{failure.path}</strong><span>{failure.reason}</span></p>
      {/each}
    </details>
  {/if}
</section>
