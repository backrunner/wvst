<script lang="ts">
  import ArrowRight from 'phosphor-svelte/lib/ArrowRight';
  import Waveform from 'phosphor-svelte/lib/Waveform';
  import SpeakerHigh from 'phosphor-svelte/lib/SpeakerHigh';
  import type { DemoCopy, RackSlot } from './types';
  export let t: DemoCopy;
  export let locale: 'en' | 'zh';
  export let rack: RackSlot[];
  export let fileName: string;
  export let levels: [number, number];
  export let isPlaying: boolean;
</script>

<section class="studio-signal" aria-label={t.signalPath}>
  <div class="signal-title"><span>{t.signalPath}</span><small>{t.listenHint}</small></div>
  <div class="signal-chain">
    <span class="signal-node" title={fileName}><Waveform size={17} />{fileName || t.source}</span><ArrowRight size={17} aria-hidden="true" />
    {#each rack.filter(slot => slot.state === 'active') as slot}
      <span class="signal-node effect" class:bypassed={slot.bypassed} title={slot.choice.label}>{slot.choice.label}{#if slot.bypassed}<small>{t.effectBypassed}</small>{/if}</span><ArrowRight size={17} aria-hidden="true" />
    {:else}<span class="signal-direct">{t.directAudio}</span><ArrowRight size={17} aria-hidden="true" />{/each}
    <span class="signal-node"><SpeakerHigh size={17} />{t.output}</span>
    <div class="signal-meter" aria-label={t.outputHint}>{#each levels as level, i}<span><small>{i === 0 ? 'L' : 'R'}</small><b><i style={`width:${isPlaying ? level * 100 : 0}%`}></i></b></span>{/each}</div>
  </div>
</section>
<section class="studio-faq" aria-label={t.supportTitle}>
  <div><h2>{t.supportTitle}</h2><a class="studio-inline-link" href={locale === 'zh' ? '/docs/zh/demo-guide' : '/docs/demo-guide'}>{t.guideLink} ↗</a></div>
  <div>{#each [[t.helpNoSound,t.helpNoSoundBody],[t.helpPlugins,t.helpPluginsBody],[t.helpLatency,t.helpLatencyBody]] as item}
    <details><summary>{item[0]}</summary><p>{item[1]}</p></details>
  {/each}</div>
</section>
