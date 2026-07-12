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
  export let levels: [number, number] = [0, 0];
  export let activeSlots = 0;
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

<section class="wvst-deck" aria-label={t.chooseFile} data-playing={isPlaying}>
  <div class="wvst-tape-window" aria-hidden="true">
    <span class="wvst-reel"><i></i></span>
    <span class="wvst-tape-run"><i></i></span>
    <span class="wvst-reel"><i></i></span>
  </div>

  <button
    class:dragging
    class="wvst-file-zone"
    type="button"
    on:click={onChooseFile}
    on:dragenter|preventDefault={() => (dragging = true)}
    on:dragover|preventDefault={() => (dragging = true)}
    on:dragleave={() => (dragging = false)}
    on:drop|preventDefault={(event) => { dragging = false; onDropFile(event); }}
  >
    {#if fileName}
      <FileAudio size={24} weight="duotone" />
      <span><strong>{fileName}</strong><small>{t.replaceFile}</small></span>
    {:else}
      <UploadSimple size={24} weight="duotone" />
      <span><strong>{t.dropFile}</strong><small>{t.chooseFile}</small></span>
    {/if}
  </button>

  <input bind:this={fileInput} class="wvst-file-input" type="file" accept="audio/*" on:change={onFileSelected} />
  <audio
    bind:this={mediaElement}
    src={fileUrl}
    preload="metadata"
    {loop}
    on:timeupdate={onTimeUpdate}
    on:loadedmetadata={onTimeUpdate}
    on:ended={onEnded}
  ></audio>

  <div class="wvst-deck-display">
    <span>{activeSlots > 0 ? t.processedPath : t.dryPath}</span>
    <strong>{formatTime(currentTime)} / {formatTime(duration)}</strong>
  </div>

  <label class="wvst-scrubber">
    <span class="wvst-visually-hidden">{t.play}</span>
    <input
      type="range"
      min="0"
      max={duration || 0}
      step="0.01"
      value={currentTime}
      disabled={!duration}
      aria-valuetext={`${formatTime(currentTime)} / ${formatTime(duration)}`}
      on:input={onSeek}
    />
  </label>

  <div class="wvst-transport" aria-label={t.play}>
    <button type="button" title={t.back} aria-label={t.back} disabled={!fileUrl} on:click={() => onSkip(-10)}><Rewind size={20} weight="fill" /></button>
    <button class="wvst-play" type="button" title={isPlaying ? t.pause : t.play} aria-label={isPlaying ? t.pause : t.play} disabled={!fileUrl} on:click={onTogglePlayback}>
      {#if isPlaying}<Pause size={23} weight="fill" />{:else}<Play size={23} weight="fill" />{/if}
    </button>
    <button type="button" title={t.stop} aria-label={t.stop} disabled={!fileUrl} on:click={onStop}><Stop size={19} weight="fill" /></button>
    <button type="button" title={t.forward} aria-label={t.forward} disabled={!fileUrl} on:click={() => onSkip(10)}><FastForward size={20} weight="fill" /></button>
    <button class:active={loop} type="button" title={t.loop} aria-label={t.loop} aria-pressed={loop} on:click={onToggleLoop}><Repeat size={20} weight="bold" /></button>
  </div>

  <div class="wvst-output-strip">
    <button class="wvst-icon-button" type="button" title={muted ? t.unmute : t.mute} aria-label={muted ? t.unmute : t.mute} aria-pressed={muted} on:click={onToggleMute}>
      {#if muted}<SpeakerX size={20} />{:else}<SpeakerHigh size={20} />{/if}
    </button>
    <label class="wvst-volume">
      <span>{t.volume}</span>
      <input type="range" min="0" max="1" step="0.01" value={volume} aria-label={t.volume} on:input={onVolume} />
    </label>
    <div class="wvst-stereo-meter" aria-label={t.meters}>
      <span><i style={`transform:scaleX(${Math.max(.025, levels[0])})`}></i></span>
      <span><i style={`transform:scaleX(${Math.max(.025, levels[1])})`}></i></span>
    </div>
  </div>
</section>
