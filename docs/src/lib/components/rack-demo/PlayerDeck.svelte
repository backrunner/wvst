<script lang="ts">
  import FastForward from 'phosphor-svelte/lib/FastForward';
  import FileAudio from 'phosphor-svelte/lib/FileAudio';
  import Pause from 'phosphor-svelte/lib/Pause';
  import Play from 'phosphor-svelte/lib/Play';
  import Repeat from 'phosphor-svelte/lib/Repeat';
  import Rewind from 'phosphor-svelte/lib/Rewind';
  import SpeakerHigh from 'phosphor-svelte/lib/SpeakerHigh';
  import SpeakerX from 'phosphor-svelte/lib/SpeakerX';
  import Stop from 'phosphor-svelte/lib/Stop';
  import UploadSimple from 'phosphor-svelte/lib/UploadSimple';
  import type { DemoCopy } from './types';

  export let t: DemoCopy;
  export let mediaElement: HTMLAudioElement | undefined = undefined;
  export let fileInput: HTMLInputElement | undefined = undefined;
  export let fileUrl: string | undefined = undefined;
  export let fileName = '';
  export let isPlaying = false;
  export let currentTime = 0;
  export let duration = 0;
  export let volume = .82;
  export let muted = false;
  export let loop = false;
  export let activeSlots = 0;
  export let peaks: number[] = [];
  export let waveformBusy = false;
  export let onSample: () => void;
  export let onAudioError: () => void;
  $: progress = duration > 0 ? currentTime / duration : 0;
  $: peakMax = Math.max(.01, ...peaks);
  export let onChooseFile: () => void;
  export let onFileSelected: (event: Event) => void;
  export let onDropFile: (event: DragEvent) => void;
  export let onTogglePlayback: () => void;
  export let onStop: () => void;
  export let onSkip: (seconds: number) => void;
  export let onSeek: (event: Event) => void;
  export let onVolume: (event: Event) => void;
  export let onToggleMute: () => void;
  export let onToggleLoop: () => void;
  export let onTimeUpdate: () => void;
  export let onEnded: () => void;

  let dragging = false;

  function formatTime(value: number): string {
    if (!Number.isFinite(value) || value <= 0) return '0:00';
    const minutes = Math.floor(value / 60);
    const seconds = Math.floor(value % 60).toString().padStart(2, '0');
    return `${minutes}:${seconds}`;
  }
</script>

<section id="studio-source" class="wvst-deck" aria-label={t.source}>
  <header class="wvst-panel-header"><div><span class="panel-index">01 / {t.source}</span><h2>{t.sourceCaption}</h2></div><FileAudio size={23} /></header>
  <div class="player-screen" class:loaded={Boolean(fileUrl)}>
    <div class="screen-top"><span><i class:playing={isPlaying}></i>{isPlaying ? t.nowPlaying : fileUrl ? t.paused : t.noFile}</span><span>STEREO</span></div>
    <strong class="screen-filename" title={fileName}>{fileName || '— — —'}</strong>
    <div class="player-waveform" aria-hidden="true">
      {#if peaks.length > 0}
        {#each peaks as peak, index}<i class:played={index / peaks.length < progress} style={`height:${Math.max(2, peak / peakMax * 100)}%`}></i>{/each}
      {:else}<div class="waveform-placeholder"><span></span><small>{waveformBusy ? t.sampleLoading : t.waveformEmpty}</small></div>{/if}
    </div>
    <div class="screen-time"><strong>{formatTime(currentTime)}</strong><span>/ {formatTime(duration)}</span><small>{activeSlots > 0 ? t.processedAudio : t.directAudio}</small></div>
  </div>

  <label class="wvst-scrubber"><span class="wvst-visually-hidden">{t.seek}</span>
    <input type="range" min="0" max={duration || 0} step="0.01" value={currentTime} disabled={!duration} aria-valuetext={`${formatTime(currentTime)} / ${formatTime(duration)}`} on:input={onSeek} />
  </label>
  <div class="wvst-transport" aria-label={t.play}>
    <button type="button" title={t.back} aria-label={t.back} disabled={!fileUrl} on:click={() => onSkip(-10)}><Rewind size={19} weight="fill" /></button>
    <button type="button" title={t.stop} aria-label={t.stop} disabled={!fileUrl} on:click={onStop}><Stop size={18} weight="fill" /></button>
    <button class="wvst-play" type="button" title={isPlaying ? t.pause : t.play} aria-label={isPlaying ? t.pause : t.play} disabled={!fileUrl} on:click={onTogglePlayback}>
      {#if isPlaying}<Pause size={23} weight="fill" />{:else}<Play size={23} weight="fill" />{/if}
    </button>
    <button type="button" title={t.forward} aria-label={t.forward} disabled={!fileUrl} on:click={() => onSkip(10)}><FastForward size={19} weight="fill" /></button>
    <button class:active={loop} type="button" title={t.loop} aria-label={t.loop} aria-pressed={loop} on:click={onToggleLoop}><Repeat size={21} /></button>
  </div>
  <div class="wvst-output-strip">
    <button class="wvst-icon-button" type="button" title={muted ? t.unmute : t.mute} aria-label={muted ? t.unmute : t.mute} aria-pressed={muted} on:click={onToggleMute}>{#if muted}<SpeakerX size={19} />{:else}<SpeakerHigh size={19} />{/if}</button>
    <label class="wvst-volume"><span class="wvst-visually-hidden">{t.volume}</span><input type="range" min="0" max="1" step="0.01" value={volume} on:input={onVolume} /></label>
    <output>{Math.round(volume * 100)}%</output>
  </div>

  <button class="wvst-file-zone" class:dragging type="button" on:click={onChooseFile}
    on:dragenter|preventDefault={() => dragging = true} on:dragover|preventDefault={() => dragging = true}
    on:dragleave={() => dragging = false} on:drop|preventDefault={(event) => { dragging = false; onDropFile(event); }}>
    <UploadSimple size={23} /><span><strong>{fileUrl ? t.replaceFile : t.dropFile}</strong><small>{t.fileHint}</small></span><span class="file-plus">+</span>
  </button>
  <button class="player-sample" type="button" on:click={onSample}><Play size={15} /><span>{t.sample}</span><small>{t.sampleHint}</small></button>
  <p class="player-privacy">{t.uploadPrivacy}</p>
  <input bind:this={fileInput} class="wvst-file-input" type="file" accept="audio/*" on:change={onFileSelected} />
  <audio bind:this={mediaElement} src={fileUrl} preload="metadata" {loop} on:timeupdate={onTimeUpdate} on:loadedmetadata={onTimeUpdate} on:ended={onEnded} on:error={onAudioError}></audio>
</section>
