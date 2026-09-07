<script lang="ts">
  import Check from 'phosphor-svelte/lib/Check';
  import ArrowUpRight from 'phosphor-svelte/lib/ArrowUpRight';
  import type { DemoCopy } from './types';
  export let t: DemoCopy;
  export let connected: boolean;
  export let hasAudio: boolean;
  export let hasEffect: boolean;
  $: steps = [
    { title: t.stepBridge, hint: t.stepBridgeHint, done: connected, href: '#studio-connect' },
    { title: t.stepAudio, hint: t.stepAudioHint, done: hasAudio, href: '#studio-source' },
    { title: t.stepEffect, hint: t.stepEffectHint, done: hasEffect, href: '#studio-rack' }
  ];
</script>

<nav class="studio-steps" aria-label={t.guide}>
  {#each steps as step, index}
    <a href={step.href} class:done={step.done}>
      <span class="step-number">{#if step.done}<Check size={17} weight="bold" />{:else}0{index + 1}{/if}</span>
      <span><strong>{step.title}</strong><small>{step.hint}</small></span>
      <ArrowUpRight size={16} aria-hidden="true" />
    </a>
  {/each}
</nav>
