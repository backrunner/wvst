<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Button, FormField, Input, Select } from 'svedocs/theme';
  import {
    WVSTBridgeWorkerClient,
    WVSTClient,
    configureLoopbackAudioWorkletNode,
    createLoopbackSharedBuffers,
    readLoopbackMetrics,
    type InstanceDescriptor,
    type LoopbackMetrics,
    type LoopbackSharedBuffers,
    type PluginDescriptor
  } from '@wvst/web';
  import BridgeWorker from '@wvst/web/bridge-worker?worker';
  import loopbackProcessorUrl from '@wvst/web/loopback-processor?url';

  export let locale: 'en' | 'zh' = 'en';

  type DemoStatus = 'idle' | 'blocked' | 'connecting' | 'connected' | 'scanning' | 'error';
  type SlotState = 'active' | 'failed' | 'removing';

  interface Prerequisites {
    sharedArrayBuffer: boolean;
    crossOriginIsolated: boolean;
    secureContext: boolean;
  }

  interface PluginChoice {
    key: string;
    pluginId: string;
    classId?: string;
    label: string;
    sublabel: string;
    path: string;
    score: number;
  }

  interface RackSlot {
    id: string;
    choice: PluginChoice;
    instance: InstanceDescriptor;
    node: AudioWorkletNode;
    buffers: LoopbackSharedBuffers;
    metrics: LoopbackMetrics;
    bypassed: boolean;
    state: SlotState;
    error?: string;
  }

  const text = {
    en: {
      chooseFile: 'Audio',
      noFile: 'No audio',
      connect: 'Connect',
      disconnect: 'Disconnect',
      scan: 'Scan',
      add: 'Mount',
      play: 'Play',
      pause: 'Pause',
      endpoint: 'Endpoint',
      token: 'Token',
      tokenHint: 'Optional',
      selectPlugin: 'Effect VST',
      emptyRack: 'Rack empty',
      noPlugins: 'No plugins',
      connected: 'Bridge connected',
      blocked: 'Low-latency prerequisites missing.',
      bridgeError: 'Bridge failed.',
      fileReady: 'Audio loaded',
      mounted: 'Mounted',
      removed: 'Removed',
      meters: 'Metrics',
      bypass: 'Bypass',
      enable: 'Enable',
      remove: 'Remove',
      up: 'Up',
      down: 'Down',
      status: 'Status',
      failures: 'Scan failures',
      dryPath: 'Dry',
      processedPath: 'WVST',
      volume: 'Volume'
    },
    zh: {
      chooseFile: '音频',
      noFile: '未选择音频',
      connect: '连接',
      disconnect: '断开',
      scan: '扫描',
      add: '挂载',
      play: '播放',
      pause: '暂停',
      endpoint: 'Endpoint',
      token: 'Token',
      tokenHint: '可选',
      selectPlugin: 'Effect VST',
      emptyRack: 'Rack 为空',
      noPlugins: '暂无插件',
      connected: 'Bridge 已连接',
      blocked: '缺少低延迟前置条件。',
      bridgeError: 'Bridge 失败。',
      fileReady: '音频已加载',
      mounted: '已挂载',
      removed: '已移除',
      meters: '指标',
      bypass: 'Bypass',
      enable: '启用',
      remove: '移除',
      up: '上移',
      down: '下移',
      status: '状态',
      failures: '扫描失败',
      dryPath: 'Dry',
      processedPath: 'WVST',
      volume: '音量'
    }
  };

  const DEFAULT_ENDPOINT = 'ws://127.0.0.1:35876';
  const FRAMES = 128;
  const CHANNELS = 2;

  let mediaElement: HTMLAudioElement | undefined;
  let fileInput: HTMLInputElement | undefined;
  let audioContext: AudioContext | undefined;
  let sourceNode: MediaElementAudioSourceNode | undefined;
  let outputGain: GainNode | undefined;
  let analyser: AnalyserNode | undefined;
  let analyserData: Uint8Array | undefined;
  let client: WVSTClient | undefined;
  let workerClient: WVSTBridgeWorkerClient | undefined;
  let worker: Worker | undefined;
  let workletModule: Promise<void> | undefined;
  let metricsTimer: number | undefined;
  let meterFrame: number | undefined;
  let fileUrl: string | undefined;

  let endpoint = DEFAULT_ENDPOINT;
  let token = '';
  let status: DemoStatus = 'idle';
  let statusMessage = '';
  let fileName = '';
  let isPlaying = false;
  let currentTime = 0;
  let duration = 0;
  let volume = 0.82;
  let level = 0;
  let plugins: PluginDescriptor[] = [];
  let scanFailures: Array<{ path: string; reason: string }> = [];
  let selectedChoiceKey = '';
  let rack: RackSlot[] = [];
  let busy = false;

  $: t = text[locale] ?? text.en;
  $: connected = Boolean(client && workerClient);
  $: pluginChoices = createPluginChoices(plugins);
  $: selectedChoice = pluginChoices.find((choice) => choice.key === selectedChoiceKey);
  $: activeSlots = rack.filter((slot) => !slot.bypassed && slot.state === 'active').length;

  onMount(() => {
    const prerequisites = readPrerequisites();
    if (!allPrerequisitesMet(prerequisites)) {
      status = 'blocked';
      statusMessage = formatPrerequisiteMessage(prerequisites);
    }
  });

  onDestroy(() => {
    void shutdown();
  });

  async function connectBridge() {
    if (busy) return;
    const prerequisites = readPrerequisites();
    if (!allPrerequisitesMet(prerequisites)) {
      status = 'blocked';
      statusMessage = formatPrerequisiteMessage(prerequisites);
      return;
    }

    busy = true;
    status = 'connecting';
    statusMessage = '';

    try {
      await ensureAudioContext();
      client = await WVSTClient.connect({
        endpoint,
        clientName: 'wvst-docs-demo',
        clientVersion: '0.1.0',
        requireLowLatency: true,
        token: token.trim() || undefined
      });
      worker = new BridgeWorker();
      workerClient = new WVSTBridgeWorkerClient({ worker });
      await workerClient.connect(endpoint);
      status = 'connected';
      statusMessage = t.connected;
      startMetricsLoop();
      startMeterLoop();
      await loadPlugins(false);
    } catch (error) {
      await closeBridgeOnly();
      status = 'error';
      statusMessage = `${t.bridgeError} ${describeError(error)}`;
    } finally {
      busy = false;
    }
  }

  async function disconnectBridge() {
    busy = true;
    try {
      await shutdown();
      status = 'idle';
      statusMessage = '';
    } finally {
      busy = false;
    }
  }

  async function loadPlugins(rescan: boolean) {
    if (!client) return;
    status = 'scanning';
    try {
      const report = rescan
        ? await client.plugins.scan()
        : await client.plugins.list({ rescan: false });
      plugins = report.plugins;
      scanFailures = report.failures;
      selectedChoiceKey = createPluginChoices(report.plugins)[0]?.key ?? '';
      status = 'connected';
      statusMessage = report.plugins.length > 0 ? t.connected : t.noPlugins;
    } catch (error) {
      status = 'error';
      statusMessage = describeError(error);
    }
  }

  function chooseFile() {
    fileInput?.click();
  }

  function handleFileSelected(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    if (fileUrl) URL.revokeObjectURL(fileUrl);
    fileUrl = URL.createObjectURL(file);
    fileName = file.name;
    currentTime = 0;
    duration = 0;
    isPlaying = false;

    if (mediaElement) {
      mediaElement.src = fileUrl;
      mediaElement.load();
    }

    statusMessage = t.fileReady;
  }

  async function togglePlayback() {
    if (!mediaElement || !fileUrl) {
      statusMessage = t.noFile;
      return;
    }

    await ensureAudioGraph();
    await audioContext?.resume();

    if (mediaElement.paused) {
      await mediaElement.play();
      isPlaying = true;
    } else {
      mediaElement.pause();
      isPlaying = false;
    }
  }

  async function mountSelectedEffect() {
    if (!client || !workerClient || !selectedChoice || busy) return;

    busy = true;
    let instance: InstanceDescriptor | undefined;
    let node: AudioWorkletNode | undefined;

    try {
      const context = await ensureAudioContext();
      await ensureAudioGraph();
      const created = await client.instances.create({
        pluginId: selectedChoice.pluginId,
        ...(selectedChoice.classId ? { classId: selectedChoice.classId } : {}),
        sampleRate: Math.round(context.sampleRate),
        maxBlockFrames: FRAMES,
        inputChannels: CHANNELS,
        outputChannels: CHANNELS
      });
      const started = await client.instances.start({ instanceId: created.instanceId });
      instance = started.instance;
      const buffers = createLoopbackSharedBuffers({
        frames: FRAMES,
        inputChannels: CHANNELS,
        outputChannels: CHANNELS,
        capacityQuanta: 4
      });
      node = await createRackNode(context, buffers);
      await workerClient.startAudioStream({
        streamId: instance.streamId,
        sampleRate: instance.sampleRate,
        frames: FRAMES,
        inputChannels: CHANNELS,
        outputChannels: CHANNELS,
        buffers
      });

      rack = [
        ...rack,
        {
          id: createId(),
          choice: selectedChoice,
          instance,
          node,
          buffers,
          metrics: readLoopbackMetrics(buffers),
          bypassed: false,
          state: 'active'
        }
      ];
      rebuildGraph();
      status = 'connected';
      statusMessage = t.mounted;
    } catch (error) {
      node?.disconnect();
      if (instance) await destroyInstance(instance);
      status = 'error';
      statusMessage = describeError(error);
    } finally {
      busy = false;
    }
  }

  async function removeSlot(slot: RackSlot) {
    rack = rack.map((item) => item.id === slot.id ? { ...item, state: 'removing' } : item);
    rebuildGraph();
    await cleanupSlot(slot);
    rack = rack.filter((item) => item.id !== slot.id);
    rebuildGraph();
    statusMessage = t.removed;
  }

  function toggleBypass(slot: RackSlot) {
    rack = rack.map((item) =>
      item.id === slot.id ? { ...item, bypassed: !item.bypassed } : item
    );
    rebuildGraph();
  }

  function moveSlot(index: number, direction: -1 | 1) {
    const nextIndex = index + direction;
    if (nextIndex < 0 || nextIndex >= rack.length) return;
    const next = [...rack];
    const [slot] = next.splice(index, 1);
    if (!slot) return;
    next.splice(nextIndex, 0, slot);
    rack = next;
    rebuildGraph();
  }

  function updateTime() {
    if (!mediaElement) return;
    currentTime = mediaElement.currentTime;
    duration = Number.isFinite(mediaElement.duration) ? mediaElement.duration : 0;
  }

  function seek(event: Event) {
    if (!mediaElement) return;
    mediaElement.currentTime = Number((event.currentTarget as HTMLInputElement).value);
    updateTime();
  }

  function changeVolume(event: Event) {
    volume = Number((event.currentTarget as HTMLInputElement).value);
    if (outputGain) outputGain.gain.value = volume;
  }

  async function ensureAudioGraph() {
    const context = await ensureAudioContext();
    if (!mediaElement) throw new Error('audio element is not ready');

    if (!sourceNode) {
      sourceNode = context.createMediaElementSource(mediaElement);
    }
    if (!outputGain) {
      outputGain = context.createGain();
      outputGain.gain.value = volume;
      analyser = context.createAnalyser();
      analyser.fftSize = 256;
      analyserData = new Uint8Array(analyser.frequencyBinCount);
      outputGain.connect(analyser);
      analyser.connect(context.destination);
    }

    rebuildGraph();
  }

  async function ensureAudioContext(): Promise<AudioContext> {
    if (audioContext) return audioContext;
    const contextCtor = window.AudioContext
      ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!contextCtor) throw new Error('AudioContext is not available');
    audioContext = new contextCtor();
    return audioContext;
  }

  function rebuildGraph() {
    if (!sourceNode || !outputGain) return;

    safeDisconnect(sourceNode);
    for (const slot of rack) safeDisconnect(slot.node);

    let tail: AudioNode = sourceNode;
    for (const slot of rack) {
      if (slot.bypassed || slot.state !== 'active') continue;
      tail.connect(slot.node);
      tail = slot.node;
    }
    tail.connect(outputGain);
  }

  async function createRackNode(
    context: AudioContext,
    buffers: LoopbackSharedBuffers
  ): Promise<AudioWorkletNode> {
    workletModule ??= context.audioWorklet.addModule(loopbackProcessorUrl);
    await workletModule;
    const node = new AudioWorkletNode(context, 'wvst-loopback', {
      numberOfInputs: 1,
      numberOfOutputs: 1,
      outputChannelCount: [CHANNELS],
      channelCount: CHANNELS,
      channelCountMode: 'explicit',
      channelInterpretation: 'speakers'
    });
    configureLoopbackAudioWorkletNode(node, buffers);
    return node;
  }

  async function cleanupSlot(slot: RackSlot) {
    slot.node.disconnect();
    if (workerClient) {
      await workerClient.stopAudioStream(slot.instance.streamId).catch(() => undefined);
    }
    await destroyInstance(slot.instance);
  }

  async function destroyInstance(instance: InstanceDescriptor) {
    if (!client) return;
    await client.instances.stop({ instanceId: instance.instanceId }).catch(() => undefined);
    await client.instances.closeStream({ instanceId: instance.instanceId }).catch(() => undefined);
    await client.instances.destroy({ instanceId: instance.instanceId }).catch(() => undefined);
  }

  async function shutdown() {
    stopMetricsLoop();
    stopMeterLoop();
    if (mediaElement) {
      mediaElement.pause();
      isPlaying = false;
    }
    for (const slot of [...rack]) {
      await cleanupSlot(slot).catch(() => undefined);
    }
    rack = [];
    plugins = [];
    scanFailures = [];
    selectedChoiceKey = '';
    await closeBridgeOnly();
    safeDisconnect(sourceNode);
    sourceNode = undefined;
    outputGain?.disconnect();
    analyser?.disconnect();
    outputGain = undefined;
    analyser = undefined;
    analyserData = undefined;
    await audioContext?.close().catch(() => undefined);
    audioContext = undefined;
    workletModule = undefined;
    if (fileUrl) URL.revokeObjectURL(fileUrl);
    fileUrl = undefined;
  }

  async function closeBridgeOnly() {
    client?.close();
    client = undefined;
    if (workerClient) {
      await workerClient.close().catch(() => undefined);
      workerClient = undefined;
    }
    worker?.terminate();
    worker = undefined;
  }

  function startMetricsLoop() {
    stopMetricsLoop();
    metricsTimer = window.setInterval(() => {
      rack = rack.map((slot) => ({
        ...slot,
        metrics: readLoopbackMetrics(slot.buffers)
      }));
    }, 500);
  }

  function stopMetricsLoop() {
    if (metricsTimer !== undefined) {
      window.clearInterval(metricsTimer);
      metricsTimer = undefined;
    }
  }

  function startMeterLoop() {
    stopMeterLoop();
    const tick = () => {
      if (analyser && analyserData) {
        analyser.getByteTimeDomainData(analyserData);
        let sum = 0;
        for (const sample of analyserData) {
          const centered = (sample - 128) / 128;
          sum += centered * centered;
        }
        level = Math.min(1, Math.sqrt(sum / analyserData.length) * 2.2);
      }
      meterFrame = window.requestAnimationFrame(tick);
    };
    tick();
  }

  function stopMeterLoop() {
    if (meterFrame !== undefined) {
      window.cancelAnimationFrame(meterFrame);
      meterFrame = undefined;
    }
  }

  function createPluginChoices(items: PluginDescriptor[]): PluginChoice[] {
    return items
      .flatMap((plugin) => {
        const classes = plugin.classes.length > 0
          ? plugin.classes
          : [{ name: plugin.name, subcategories: [] as string[], category: undefined, classId: undefined }];

        return classes.map((pluginClass, index) => {
          const category = (pluginClass.category ?? pluginClass.subcategories.join(' / ')) || 'VST3';
          const classLabel = pluginClass.name && pluginClass.name !== plugin.name
            ? `${plugin.name} - ${pluginClass.name}`
            : plugin.name;
          const classId = pluginClass.classId;
          return {
            key: `${plugin.pluginId}:${classId ?? index}`,
            pluginId: plugin.pluginId,
            ...(classId ? { classId } : {}),
            label: classLabel,
            sublabel: `${plugin.vendor ?? 'Unknown vendor'} · ${category}`,
            path: plugin.path,
            score: scorePluginChoice(`${classLabel} ${category} ${pluginClass.subcategories.join(' ')}`)
          };
        });
      })
      .sort((left, right) => left.score - right.score || left.label.localeCompare(right.label));
  }

  function scorePluginChoice(value: string): number {
    const textValue = value.toLowerCase();
    if (/(effect|fx|audio module|processor)/.test(textValue)) return 0;
    if (/(instrument|synth|generator)/.test(textValue)) return 2;
    return 1;
  }

  function readPrerequisites(): Prerequisites {
    const lowLatency = WVSTClient.lowLatencyPrerequisites();
    return {
      sharedArrayBuffer: lowLatency.sharedArrayBuffer,
      crossOriginIsolated: lowLatency.crossOriginIsolated,
      secureContext: window.isSecureContext === true
    };
  }

  function allPrerequisitesMet(value: Prerequisites): boolean {
    return value.sharedArrayBuffer && value.crossOriginIsolated && value.secureContext;
  }

  function formatPrerequisiteMessage(value: Prerequisites): string {
    const missing = [
      value.secureContext ? '' : 'secure context',
      value.crossOriginIsolated ? '' : 'cross-origin isolation',
      value.sharedArrayBuffer ? '' : 'SharedArrayBuffer'
    ].filter(Boolean);
    return `${t.blocked} Missing: ${missing.join(', ')}.`;
  }

  function describeError(error: unknown): string {
    return error instanceof Error ? error.message : String(error);
  }

  function safeDisconnect(node: AudioNode | undefined) {
    try {
      node?.disconnect();
    } catch {
      // Disconnect can throw when the graph has already been torn down.
    }
  }

  function createId(): string {
    return globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
  }

  function formatTime(value: number): string {
    if (!Number.isFinite(value) || value <= 0) return '0:00';
    const minutes = Math.floor(value / 60);
    const seconds = Math.floor(value % 60).toString().padStart(2, '0');
    return `${minutes}:${seconds}`;
  }
</script>

<section class="wvst-demo" aria-label="WVST live rack demo">
  <div class="player">
    <div class="deck">
      <div class="reel-row" aria-hidden="true">
        <span class="reel" style={`--spin:${isPlaying ? 'running' : 'paused'}`}></span>
        <span class="tape-path"></span>
        <span class="reel" style={`--spin:${isPlaying ? 'running' : 'paused'}`}></span>
      </div>

      <div class="display">
        <span>{fileName || t.noFile}</span>
        <strong>{activeSlots > 0 ? t.processedPath : t.dryPath}</strong>
      </div>

      <div class="transport">
        <Button type="button" variant="primary" density="sm" on:click={togglePlayback} disabled={!fileUrl}>
          {isPlaying ? t.pause : t.play}
        </Button>
        <Button type="button" density="sm" on:click={chooseFile}>{t.chooseFile}</Button>
      </div>

      <input
        bind:this={fileInput}
        class="file-input"
        type="file"
        accept="audio/*"
        on:change={handleFileSelected}
      />
      <audio
        bind:this={mediaElement}
        src={fileUrl}
        preload="metadata"
        on:timeupdate={updateTime}
        on:loadedmetadata={updateTime}
        on:ended={() => (isPlaying = false)}
      ></audio>

      <label class="range-label">
        <span>{formatTime(currentTime)}</span>
        <input
          type="range"
          min="0"
          max={duration || 0}
          step="0.01"
          value={currentTime}
          disabled={!duration}
          on:input={seek}
        />
        <span>{formatTime(duration)}</span>
      </label>

      <label class="volume-label">
        <span>{t.volume}</span>
        <input type="range" min="0" max="1" step="0.01" value={volume} on:input={changeVolume} />
      </label>

      <div class="vu" aria-label="Output level">
        <span style={`transform:scaleX(${Math.max(0.04, level)})`}></span>
      </div>
    </div>

    <div class="connection">
      <FormField label={t.endpoint} for="wvst-endpoint">
        <Input id="wvst-endpoint" bind:value={endpoint} density="sm" spellcheck="false" />
      </FormField>
      <FormField label={t.token} for="wvst-token">
        <Input id="wvst-token" bind:value={token} density="sm" placeholder={t.tokenHint} spellcheck="false" />
      </FormField>
      <div class="connection-actions">
        {#if connected}
          <Button type="button" density="sm" on:click={disconnectBridge} disabled={busy}>{t.disconnect}</Button>
        {:else}
          <Button type="button" variant="primary" density="sm" on:click={connectBridge} disabled={busy}>{t.connect}</Button>
        {/if}
        <Button type="button" variant="ghost" density="sm" on:click={() => loadPlugins(true)} disabled={!connected || busy}>
          {t.scan}
        </Button>
      </div>
    </div>

    <div class="status" data-state={status}>
      <strong>{t.status}</strong>
      <span>{statusMessage || status}</span>
    </div>
  </div>

  <div class="rack">
    <div class="rack-toolbar">
      <Select bind:value={selectedChoiceKey} density="sm" aria-label={t.selectPlugin} disabled={!connected || pluginChoices.length === 0}>
        <option value="">{t.selectPlugin}</option>
        {#each pluginChoices as choice}
          <option value={choice.key}>{choice.label} - {choice.sublabel}</option>
        {/each}
      </Select>
      <Button type="button" variant="primary" density="sm" on:click={mountSelectedEffect} disabled={!connected || !selectedChoice || busy}>
        {t.add}
      </Button>
    </div>

    {#if rack.length === 0}
      <div class="rack-empty">
        <strong>{pluginChoices.length === 0 ? t.noPlugins : t.emptyRack}</strong>
      </div>
    {:else}
      <ol class="slot-list">
        {#each rack as slot, index (slot.id)}
          <li class:bypassed={slot.bypassed} class:removing={slot.state === 'removing'}>
            <div class="slot-index">{index + 1}</div>
            <div class="slot-main">
              <strong>{slot.choice.label}</strong>
              <span>{slot.choice.sublabel}</span>
              <small>{slot.instance.backend ?? 'worker'} · {slot.instance.latencySamples} samples latency</small>
              {#if slot.error}<em>{slot.error}</em>{/if}
            </div>
            <div class="slot-metrics" aria-label={t.meters}>
              <span>in {slot.metrics.pendingInputQuanta}</span>
              <span>out {slot.metrics.pendingOutputQuanta}</span>
              <span>u {slot.metrics.underflows}</span>
              <span>o {slot.metrics.overflows}</span>
            </div>
            <div class="slot-actions">
              <Button type="button" variant="ghost" density="sm" on:click={() => moveSlot(index, -1)} disabled={index === 0 || busy}>{t.up}</Button>
              <Button type="button" variant="ghost" density="sm" on:click={() => moveSlot(index, 1)} disabled={index === rack.length - 1 || busy}>{t.down}</Button>
              <Button type="button" density="sm" on:click={() => toggleBypass(slot)} disabled={busy}>
                {slot.bypassed ? t.enable : t.bypass}
              </Button>
              <Button type="button" variant="danger" density="sm" on:click={() => removeSlot(slot)} disabled={busy}>{t.remove}</Button>
            </div>
          </li>
        {/each}
      </ol>
    {/if}

    {#if scanFailures.length > 0}
      <details class="failures">
        <summary>{t.failures} ({scanFailures.length})</summary>
        {#each scanFailures as failure}
          <p><strong>{failure.path}</strong><span>{failure.reason}</span></p>
        {/each}
      </details>
    {/if}
  </div>
</section>

<style>
  .wvst-demo {
    display: grid;
    grid-template-columns: minmax(17rem, 0.92fr) minmax(21rem, 1.35fr);
    gap: 1rem;
    margin: 1.6rem 0 2.2rem;
    color: var(--sd-ink);
  }

  .player,
  .rack {
    border: 1px solid color-mix(in srgb, var(--sd-line) 88%, transparent);
    background:
      linear-gradient(180deg, color-mix(in srgb, var(--sd-panel) 94%, var(--sd-bg)), var(--sd-panel));
    box-shadow: 0 18px 50px color-mix(in srgb, var(--sd-ink) 10%, transparent);
  }

  .player {
    padding: 1rem;
  }

  .deck {
    border: 1px solid color-mix(in srgb, var(--sd-ink) 24%, var(--sd-line));
    background:
      linear-gradient(90deg, color-mix(in srgb, var(--sd-line) 36%, transparent) 1px, transparent 1px),
      linear-gradient(180deg, #3b2517, #1b110b);
    background-size: 18px 18px, 100% 100%;
    color: #f7e4c9;
    padding: 1rem;
  }

  .reel-row {
    display: grid;
    grid-template-columns: 4.8rem 1fr 4.8rem;
    align-items: center;
    gap: .7rem;
    min-height: 5rem;
  }

  .reel {
    aspect-ratio: 1;
    border-radius: 50%;
    border: 10px solid #d2a064;
    background:
      radial-gradient(circle, #24140c 0 18%, transparent 19%),
      conic-gradient(from 20deg, #d2a064 0 10%, #5b3923 10% 17%, #d2a064 17% 32%, #5b3923 32% 40%, #d2a064 40%);
    animation: spin 1.5s linear infinite;
    animation-play-state: var(--spin);
  }

  .tape-path {
    height: .45rem;
    border-radius: 999px;
    background: linear-gradient(90deg, #d2a064, #5ec8c2, #d2a064);
    box-shadow: 0 0 0 1px rgba(255,255,255,.08), 0 0 24px rgba(94,200,194,.24);
  }

  .display {
    display: grid;
    gap: .25rem;
    min-height: 4.4rem;
    margin-top: .9rem;
    padding: .75rem;
    border: 1px solid rgba(242, 202, 144, .32);
    background: rgba(8, 127, 140, .12);
  }

  .display span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: .9rem;
  }

  .display strong {
    color: #87e0d8;
    font-family: var(--font-mono);
    font-size: .8rem;
    text-transform: uppercase;
  }

  .transport,
  .connection-actions,
  .rack-toolbar,
  .slot-actions {
    display: flex;
    flex-wrap: wrap;
    gap: .5rem;
  }

  .transport {
    margin-top: .85rem;
  }

  .transport :global(.sd-control-button),
  .connection-actions :global(.sd-control-button),
  .rack-toolbar :global(.sd-control-button) {
    flex: 0 0 auto;
  }

  .file-input,
  audio {
    display: none;
  }

  .range-label,
  .volume-label {
    display: grid;
    gap: .4rem;
    margin-top: .85rem;
    font-size: .82rem;
    color: var(--sd-muted);
  }

  .range-label {
    grid-template-columns: 3rem 1fr 3rem;
    align-items: center;
  }

  .range-label input,
  .volume-label input {
    width: 100%;
    box-sizing: border-box;
    min-height: 1.35rem;
    accent-color: var(--sd-accent);
    cursor: pointer;
  }

  .range-label input:disabled,
  .volume-label input:disabled {
    cursor: not-allowed;
    opacity: .48;
  }

  .vu {
    height: .8rem;
    margin-top: 1rem;
    overflow: hidden;
    border: 1px solid rgba(242, 202, 144, .28);
    background: rgba(0, 0, 0, .28);
  }

  .vu span {
    display: block;
    height: 100%;
    transform-origin: left;
    background: linear-gradient(90deg, #5ec8c2, #d2a064 72%, #f06b4f);
    transition: transform .08s linear;
  }

  .connection {
    margin-top: .9rem;
  }

  .connection :global(.sd-field) {
    margin-top: .75rem;
  }

  .connection-actions {
    margin-top: .85rem;
  }

  .status {
    display: grid;
    gap: .25rem;
    margin-top: .85rem;
    padding: .75rem;
    border-left: 3px solid var(--sd-accent);
    background: color-mix(in srgb, var(--sd-accent) 7%, var(--sd-panel));
  }

  .status[data-state="blocked"],
  .status[data-state="error"] {
    border-left-color: var(--sd-accent-2);
  }

  .status span {
    color: var(--sd-muted);
    font-size: .9rem;
  }

  .rack {
    padding: 1rem;
    min-width: 0;
  }

  .rack-toolbar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
  }

  .rack-toolbar :global(.sd-select) {
    min-width: 0;
  }

  .rack-empty {
    display: grid;
    gap: .35rem;
    min-height: 13rem;
    place-content: center;
    margin-top: .8rem;
    border: 1px dashed color-mix(in srgb, var(--sd-line) 82%, transparent);
    color: var(--sd-muted);
    text-align: center;
  }

  .rack-empty strong {
    color: var(--sd-ink);
  }

  .slot-list {
    display: grid;
    gap: .7rem;
    padding: 0;
    margin: .85rem 0 0;
    list-style: none;
  }

  .slot-list li {
    display: grid;
    grid-template-columns: 2.5rem minmax(0, 1fr);
    gap: .7rem;
    padding: .8rem;
    border: 1px solid color-mix(in srgb, var(--sd-line) 82%, transparent);
    background: color-mix(in srgb, var(--sd-bg) 40%, var(--sd-panel));
  }

  .slot-list li.bypassed {
    opacity: .65;
  }

  .slot-list li.removing {
    opacity: .42;
  }

  .slot-index {
    display: grid;
    place-items: center;
    width: 2.35rem;
    height: 2.35rem;
    border: 1px solid color-mix(in srgb, var(--sd-accent) 52%, var(--sd-line));
    color: var(--sd-accent);
    font-family: var(--font-mono);
  }

  .slot-main {
    min-width: 0;
  }

  .slot-main strong,
  .slot-main span,
  .slot-main small,
  .slot-main em {
    display: block;
  }

  .slot-main span,
  .slot-main small,
  .slot-main em {
    color: var(--sd-muted);
    font-size: .82rem;
  }

  .slot-main strong {
    overflow-wrap: anywhere;
  }

  .slot-metrics {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: .35rem;
  }

  .slot-metrics span {
    padding: .35rem;
    border: 1px solid color-mix(in srgb, var(--sd-line) 76%, transparent);
    color: var(--sd-muted);
    font-family: var(--font-mono);
    font-size: .72rem;
    text-align: center;
  }

  .slot-actions {
    grid-column: 1 / -1;
  }

  .slot-actions :global(.sd-control-button) {
    min-width: 3.25rem;
  }

  .failures {
    margin-top: .9rem;
    color: var(--sd-muted);
  }

  .failures p {
    display: grid;
    gap: .15rem;
    margin: .5rem 0;
    overflow-wrap: anywhere;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  @media (max-width: 860px) {
    .wvst-demo {
      grid-template-columns: 1fr;
    }

    .rack-toolbar {
      grid-template-columns: 1fr;
    }
  }
</style>
