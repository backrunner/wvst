<script lang="ts">
  import ArrowDown from 'phosphor-svelte/lib/ArrowDown';
  import ArrowUp from 'phosphor-svelte/lib/ArrowUp';
  import ArrowsClockwise from 'phosphor-svelte/lib/ArrowsClockwise';
  import Plug from 'phosphor-svelte/lib/Plug';
  import Power from 'phosphor-svelte/lib/Power';
  import Trash from 'phosphor-svelte/lib/Trash';
  import Warning from 'phosphor-svelte/lib/Warning';
  import { Button, Select } from 'svedocs/theme';
  import ParameterControls from './ParameterControls.svelte';
  import type { DemoCopy, PluginChoice, RackSlot } from './types';

  export let t: DemoCopy;
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

<section class="wvst-rack" aria-label={t.rackPanel}>
  <header class="wvst-panel-header">
    <div>
      <span>{t.rackPanel}</span>
      <strong>{rack.length} FX</strong>
    </div>
    <button class="wvst-icon-button" type="button" title={t.scan} aria-label={t.scan} on:click={onScan} disabled={!connected || busy}>
      <ArrowsClockwise size={19} class={busy ? 'spinning' : ''} />
    </button>
  </header>

  <div class="wvst-rack-toolbar">
    <Select bind:value={selectedChoiceKey} density="sm" aria-label={t.selectPlugin} disabled={!connected || pluginChoices.length === 0}>
      <option value="">{t.selectPlugin}</option>
      {#each pluginChoices as choice}
        <option value={choice.key}>{choice.label} - {choice.sublabel}</option>
      {/each}
    </Select>
    <Button type="button" variant="primary" density="sm" on:click={onMountEffect} disabled={!connected || !selected || busy}>
      <Plug size={17} weight="bold" />{t.add}
    </Button>
  </div>

  {#if rack.length === 0}
    <div class="wvst-rack-empty">
      <Plug size={34} weight="duotone" />
      <strong>{pluginChoices.length === 0 ? t.noPlugins : t.emptyRack}</strong>
      <p>{t.emptyRackBody}</p>
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
            </div>
            <div class="wvst-slot-actions">
              <button type="button" title={t.up} aria-label={t.up} on:click={() => onMove(index, -1)} disabled={index === 0 || busy}><ArrowUp size={17} /></button>
              <button type="button" title={t.down} aria-label={t.down} on:click={() => onMove(index, 1)} disabled={index === rack.length - 1 || busy}><ArrowDown size={17} /></button>
              <button class:active={!slot.bypassed} type="button" title={slot.bypassed ? t.enable : t.bypass} aria-label={slot.bypassed ? t.enable : t.bypass} aria-pressed={!slot.bypassed} on:click={() => onBypass(slot)} disabled={busy}><Power size={17} weight="bold" /></button>
              <button class="danger" type="button" title={t.remove} aria-label={t.remove} on:click={() => onRemove(slot)} disabled={busy}><Trash size={17} /></button>
            </div>
          </div>

          <div class="wvst-slot-metrics" aria-label={t.meters}>
            <span><small>{t.inputQueue}</small><strong>{slot.metrics.pendingInputQuanta}</strong></span>
            <span><small>{t.outputQueue}</small><strong>{slot.metrics.pendingOutputQuanta}</strong></span>
            <span><small>{t.underflows}</small><strong>{slot.metrics.underflows}</strong></span>
            <span><small>{t.overflows}</small><strong>{slot.metrics.overflows}</strong></span>
            <span><small>{t.latency}</small><strong>{slot.instance.latencySamples} smp</strong></span>
          </div>

          <ParameterControls
            {t}
            slotId={slot.id}
            parameters={slot.parameters}
            disabled={busy || slot.bypassed}
            onPreview={onPreviewParameter}
            onCommit={onCommitParameter}
          />
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
