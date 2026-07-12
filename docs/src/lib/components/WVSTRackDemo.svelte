<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import Check from 'phosphor-svelte/lib/Check';
  import PlugsConnected from 'phosphor-svelte/lib/PlugsConnected';
  import WarningCircle from 'phosphor-svelte/lib/WarningCircle';
  import X from 'phosphor-svelte/lib/X';
  import { Button, FormField, Input } from 'svedocs/theme';
  import {
    WVSTBridgeWorkerClient,
    WVSTClient,
    configureLoopbackAudioWorkletNode,
    createLoopbackSharedBuffers,
    readLoopbackMetrics,
    type InstanceDescriptor,
    type LoopbackSharedBuffers,
    type PluginDescriptor
  } from '@wvst/web';
  import BridgeWorker from '@wvst/web/bridge-worker?worker';
  import loopbackProcessorUrl from '@wvst/web/loopback-processor?url';
  import EffectRack from './rack-demo/EffectRack.svelte';
  import PlayerDeck from './rack-demo/PlayerDeck.svelte';
  import { demoCopy } from './rack-demo/copy';
  import type { DemoStatus, Prerequisites, RackParameter, RackSlot } from './rack-demo/types';
  import { allPrerequisitesMet, createId, createPluginChoices, describeError, formatParameterValue,
    readPrerequisites, safeDisconnect, updateRackParameter } from './rack-demo/utils';

  export let locale: 'en' | 'zh' = 'en';

  const DEFAULT_ENDPOINT = 'ws://127.0.0.1:35876';
  const FRAMES = 128, CHANNELS = 2;
  const MAX_VISIBLE_PARAMETERS = 8;

  let mediaElement: HTMLAudioElement | undefined;
  let fileInput: HTMLInputElement | undefined;
  let audioContext: AudioContext | undefined;
  let sourceNode: MediaElementAudioSourceNode | undefined;
  let outputGain: GainNode | undefined;
  let splitter: ChannelSplitterNode | undefined;
  let merger: ChannelMergerNode | undefined;
  let analysers: AnalyserNode[] = [];
  let analyserData: Uint8Array<ArrayBuffer>[] = [];
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
  let volume = .82;
  let muted = false;
  let loop = false;
  let levels: [number, number] = [0, 0];
  let plugins: PluginDescriptor[] = [];
  let scanFailures: Array<{ path: string; reason: string }> = [];
  let selectedChoiceKey = '';
  let rack: RackSlot[] = [];
  let busy = false;
  let disposed = false;
  let prerequisites: Prerequisites = { secureContext: false, crossOriginIsolated: false, sharedArrayBuffer: false };

  $: t = demoCopy[locale] ?? demoCopy.en;
  $: connected = Boolean(client && workerClient);
  $: pluginChoices = createPluginChoices(plugins);
  $: selectedChoice = pluginChoices.find((choice) => choice.key === selectedChoiceKey);
  $: activeSlots = rack.filter((slot) => !slot.bypassed && slot.state === 'active').length;
  onMount(() => {
    prerequisites = readPrerequisites();
    if (!allPrerequisitesMet(prerequisites)) {
      status = 'blocked';
      statusMessage = formatPrerequisiteMessage(prerequisites);
      return;
    }
    void connectBridge();
  });

  onDestroy(() => {
    disposed = true;
    void shutdown();
  });

  async function connectBridge() {
    if (busy) return;
    prerequisites = readPrerequisites();
    if (!allPrerequisitesMet(prerequisites)) {
      status = 'blocked';
      statusMessage = formatPrerequisiteMessage(prerequisites);
      return;
    }

    const targetEndpoint = endpoint.trim();
    const targetToken = token.trim();
    busy = true;
    status = 'connecting';
    statusMessage = t.connecting;
    try {
      client = await WVSTClient.connect({
        endpoint: targetEndpoint,
        clientName: 'wvst-docs-demo',
        clientVersion: '0.1.0',
        requireLowLatency: true,
        token: targetToken || undefined
      });
      if (disposed) {
        await closeBridgeOnly();
        return;
      }
      worker = new BridgeWorker();
      workerClient = new WVSTBridgeWorkerClient({ worker });
      await workerClient.connect(targetEndpoint);
      if (disposed) {
        await closeBridgeOnly();
        return;
      }
      status = 'connected';
      statusMessage = t.connected;
      startMetricsLoop();
      await loadPlugins(false);
    } catch (error) {
      await closeBridgeOnly();
      if (!disposed) {
        status = 'error';
        statusMessage = `${t.bridgeError} ${describeError(error)}`;
      }
    } finally {
      if (!disposed) busy = false;
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
    statusMessage = t.scanning;
    try {
      const report = rescan ? await client.plugins.scan() : await client.plugins.list({ rescan: false });
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
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (file) loadFile(file);
  }

  function handleFileDrop(event: DragEvent) {
    const file = event.dataTransfer?.files?.[0];
    if (file) loadFile(file);
  }

  function loadFile(file: File) {
    if (!file.type.startsWith('audio/') && !/\.(aac|aif|aiff|flac|m4a|mp3|ogg|wav)$/i.test(file.name)) {
      status = 'error';
      statusMessage = t.invalidFile;
      return;
    }
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
    if (!mediaElement || !fileUrl) return;
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

  function stopPlayback() {
    if (!mediaElement) return;
    mediaElement.pause();
    mediaElement.currentTime = 0;
    isPlaying = false;
    updateTime();
  }

  function skip(seconds: number) {
    if (!mediaElement) return;
    mediaElement.currentTime = Math.min(duration || Infinity, Math.max(0, mediaElement.currentTime + seconds));
    updateTime();
  }

  function seek(event: Event) {
    if (!mediaElement) return;
    mediaElement.currentTime = Number((event.currentTarget as HTMLInputElement).value);
    updateTime();
  }

  function updateTime() {
    if (!mediaElement) return;
    currentTime = mediaElement.currentTime;
    duration = Number.isFinite(mediaElement.duration) ? mediaElement.duration : 0;
  }

  function changeVolume(event: Event) {
    volume = Number((event.currentTarget as HTMLInputElement).value);
    updateOutputGain();
  }

  function toggleMute() {
    muted = !muted;
    updateOutputGain();
  }

  function toggleLoop() {
    loop = !loop;
    if (mediaElement) mediaElement.loop = loop;
  }

  function updateOutputGain() {
    if (!outputGain || !audioContext) return;
    outputGain.gain.setTargetAtTime(muted ? 0 : volume, audioContext.currentTime, .012);
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
      instance = (await client.instances.start({ instanceId: created.instanceId })).instance;
      const buffers = createLoopbackSharedBuffers({ frames: FRAMES, inputChannels: CHANNELS, outputChannels: CHANNELS, capacityQuanta: 4 });
      node = await createRackNode(context, buffers);
      await workerClient.startAudioStream({
        streamId: instance.streamId,
        sampleRate: instance.sampleRate,
        frames: FRAMES,
        inputChannels: CHANNELS,
        outputChannels: CHANNELS,
        buffers
      });
      const parameters = await loadParameters(instance);
      rack = [...rack, {
        id: createId(), choice: selectedChoice, instance, node, buffers,
        metrics: readLoopbackMetrics(buffers), parameters, bypassed: false, state: 'active'
      }];
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

  async function loadParameters(instance: InstanceDescriptor): Promise<RackParameter[]> {
    if (!client || !instance.runtimeCapabilities.parameters) return [];
    try {
      const result = await client.instances.parameters({ instanceId: instance.instanceId });
      const visible = result.parameters.filter((parameter) => !parameter.flags.hidden && !parameter.flags.readOnly).slice(0, MAX_VISIBLE_PARAMETERS);
      return Promise.all(visible.map(async (info) => {
        const current = await client!.instances.parameterInfo({ instanceId: instance.instanceId, parameterId: info.id });
        return { info, value: current.valueNormalized, display: current.valueString ?? formatParameterValue(current.valueNormalized, info.units), pending: false };
      }));
    } catch {
      return [];
    }
  }

  function previewParameter(slotId: string, parameterId: number, value: number) {
    rack = updateRackParameter(rack, slotId, parameterId, (parameter) => ({
      ...parameter,
      value,
      display: formatParameterValue(value, parameter.info.units)
    }));
  }

  async function commitParameter(slotId: string, parameterId: number, value: number) {
    const slot = rack.find((item) => item.id === slotId);
    if (!client || !slot) return;
    rack = updateRackParameter(rack, slotId, parameterId, (parameter) => ({ ...parameter, pending: true }));
    try {
      await client.instances.parameterEdit({ instanceId: slot.instance.instanceId, parameterId, valueNormalized: value });
      const current = await client.instances.parameterInfo({ instanceId: slot.instance.instanceId, parameterId, valueNormalized: value });
      rack = updateRackParameter(rack, slotId, parameterId, (parameter) => ({
        ...parameter, value: current.valueNormalized,
        display: current.valueString ?? formatParameterValue(current.valueNormalized, parameter.info.units), pending: false
      }));
    } catch (error) {
      rack = updateRackParameter(rack, slotId, parameterId, (parameter) => ({ ...parameter, pending: false }));
      status = 'error';
      statusMessage = describeError(error);
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
    rack = rack.map((item) => item.id === slot.id ? { ...item, bypassed: !item.bypassed } : item);
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

  async function ensureAudioContext(): Promise<AudioContext> {
    if (audioContext) return audioContext;
    const contextCtor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!contextCtor) throw new Error('AudioContext is not available');
    audioContext = new contextCtor();
    return audioContext;
  }

  async function ensureAudioGraph() {
    const context = await ensureAudioContext();
    if (!mediaElement) throw new Error('Audio element is not ready');
    sourceNode ??= context.createMediaElementSource(mediaElement);
    if (!outputGain) {
      outputGain = context.createGain();
      splitter = context.createChannelSplitter(CHANNELS);
      merger = context.createChannelMerger(CHANNELS);
      analysers = [context.createAnalyser(), context.createAnalyser()];
      analyserData = analysers.map((analyser) => {
        analyser.fftSize = 256;
        return new Uint8Array(analyser.frequencyBinCount);
      });
      outputGain.connect(splitter);
      splitter.connect(analysers[0]!, 0); splitter.connect(analysers[1]!, 1);
      analysers[0]!.connect(merger, 0, 0); analysers[1]!.connect(merger, 0, 1);
      merger.connect(context.destination);
      updateOutputGain();
      startMeterLoop();
    }
    rebuildGraph();
  }

  function rebuildGraph() {
    if (!sourceNode || !outputGain) return;
    safeDisconnect(sourceNode);
    rack.forEach((slot) => safeDisconnect(slot.node));
    let tail: AudioNode = sourceNode;
    for (const slot of rack) {
      if (slot.bypassed || slot.state !== 'active') continue;
      tail.connect(slot.node);
      tail = slot.node;
    }
    tail.connect(outputGain);
  }

  async function createRackNode(context: AudioContext, buffers: LoopbackSharedBuffers): Promise<AudioWorkletNode> {
    workletModule ??= context.audioWorklet.addModule(loopbackProcessorUrl);
    await workletModule;
    const node = new AudioWorkletNode(context, 'wvst-loopback', {
      numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [CHANNELS], channelCount: CHANNELS,
      channelCountMode: 'explicit', channelInterpretation: 'speakers'
    });
    configureLoopbackAudioWorkletNode(node, buffers);
    return node;
  }

  async function cleanupSlot(slot: RackSlot) {
    slot.node.disconnect();
    if (workerClient) await workerClient.stopAudioStream(slot.instance.streamId).catch(() => undefined);
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
    mediaElement?.pause();
    isPlaying = false;
    for (const slot of [...rack]) await cleanupSlot(slot).catch(() => undefined);
    rack = [];
    plugins = [];
    scanFailures = [];
    selectedChoiceKey = '';
    await closeBridgeOnly();
    safeDisconnect(sourceNode);
    outputGain?.disconnect();
    splitter?.disconnect();
    merger?.disconnect();
    analysers.forEach((node) => node.disconnect());
    sourceNode = undefined;
    outputGain = undefined;
    splitter = undefined;
    merger = undefined;
    analysers = [];
    analyserData = [];
    await audioContext?.close().catch(() => undefined);
    audioContext = undefined;
    workletModule = undefined;
    if (fileUrl) URL.revokeObjectURL(fileUrl);
    fileUrl = undefined;
    fileName = '';
  }

  async function closeBridgeOnly() {
    client?.close();
    client = undefined;
    if (workerClient) await workerClient.close().catch(() => undefined);
    workerClient = undefined;
    worker?.terminate();
    worker = undefined;
  }

  function startMetricsLoop() {
    stopMetricsLoop();
    metricsTimer = window.setInterval(() => {
      rack = rack.map((slot) => ({ ...slot, metrics: readLoopbackMetrics(slot.buffers) }));
    }, 500);
  }

  function stopMetricsLoop() {
    if (metricsTimer === undefined) return;
    window.clearInterval(metricsTimer);
    metricsTimer = undefined;
  }

  function startMeterLoop() {
    stopMeterLoop();
    const tick = () => {
      levels = [0, 1].map((index) => readLevel(analysers[index], analyserData[index])) as [number, number];
      meterFrame = window.requestAnimationFrame(tick);
    };
    tick();
  }

  function stopMeterLoop() {
    if (meterFrame === undefined) return;
    window.cancelAnimationFrame(meterFrame);
    meterFrame = undefined;
  }

  function readLevel(analyser: AnalyserNode | undefined, data: Uint8Array<ArrayBuffer> | undefined): number {
    if (!analyser || !data) return 0;
    analyser.getByteTimeDomainData(data);
    let sum = 0;
    for (const sample of data) {
      const centered = (sample - 128) / 128;
      sum += centered * centered;
    }
    return Math.min(1, Math.sqrt(sum / data.length) * 2.4);
  }

  function formatPrerequisiteMessage(value: Prerequisites): string {
    const missing = [value.secureContext ? '' : t.secureContext, value.crossOriginIsolated ? '' : t.isolation, value.sharedArrayBuffer ? '' : t.sharedBuffer].filter(Boolean);
    return `${t.blocked} ${missing.join(', ')}`;
  }

</script>

<section class="wvst-demo" aria-label="WVST live rack demo">
  <header class="wvst-demo-header">
    <div>
      <span>WVST LIVE RACK</span>
      <strong>{fileName || t.noFile}</strong>
    </div>
    <div class="wvst-runtime-badge" data-state={status}>
      {#if status === 'error' || status === 'blocked'}<WarningCircle size={17} />{:else}<PlugsConnected size={17} />{/if}
      <span>{statusMessage || status}</span>
    </div>
  </header>

  <div class="wvst-demo-layout">
    <PlayerDeck
      {t}
      bind:mediaElement
      bind:fileInput
      {fileUrl}
      {fileName}
      {isPlaying}
      {currentTime}
      {duration}
      {volume}
      {muted}
      {loop}
      {levels}
      {activeSlots}
      onChooseFile={chooseFile}
      onFileSelected={handleFileSelected}
      onDropFile={handleFileDrop}
      onTogglePlayback={togglePlayback}
      onStop={stopPlayback}
      onSkip={skip}
      onSeek={seek}
      onVolume={changeVolume}
      onToggleMute={toggleMute}
      onToggleLoop={toggleLoop}
      onTimeUpdate={updateTime}
      onEnded={() => (isPlaying = false)}
    />

    <div class="wvst-demo-right">
      <section class="wvst-bridge-panel" aria-label={t.bridgePanel}>
        <div class="wvst-readiness">
          {#each [
            [t.secureContext, prerequisites.secureContext],
            [t.isolation, prerequisites.crossOriginIsolated],
            [t.sharedBuffer, prerequisites.sharedArrayBuffer]
          ] as item}
            <span class:ready={item[1]}>{#if item[1]}<Check size={15} weight="bold" />{:else}<X size={15} weight="bold" />{/if}{item[0]}</span>
          {/each}
        </div>

        <details class="wvst-connection-settings" open={!connected}>
          <summary>{t.settings}</summary>
          <div class="wvst-connection-fields">
            <FormField label={t.endpoint} for="wvst-endpoint">
              <Input id="wvst-endpoint" bind:value={endpoint} density="sm" spellcheck="false" disabled={connected || busy} />
            </FormField>
            <FormField label={t.token} for="wvst-token">
              <Input id="wvst-token" bind:value={token} density="sm" placeholder={t.tokenHint} spellcheck="false" disabled={connected || busy} />
            </FormField>
          </div>
        </details>

        <div class="wvst-connection-actions">
          {#if connected}
            <Button type="button" density="sm" on:click={disconnectBridge} disabled={busy}>{t.disconnect}</Button>
          {:else}
            <Button type="button" variant="primary" density="sm" on:click={connectBridge} disabled={busy || status === 'blocked'}>{t.connect}</Button>
          {/if}
          <span>{connected ? t.connected : status === 'connecting' ? t.connecting : t.status}</span>
        </div>
      </section>

      <EffectRack
        {t}
        {pluginChoices}
        bind:selectedChoiceKey
        {connected}
        {busy}
        {rack}
        {scanFailures}
        onMountEffect={mountSelectedEffect}
        onScan={() => loadPlugins(true)}
        onMove={moveSlot}
        onBypass={toggleBypass}
        onRemove={removeSlot}
        onPreviewParameter={previewParameter}
        onCommitParameter={commitParameter}
      />
    </div>
  </div>
</section>
