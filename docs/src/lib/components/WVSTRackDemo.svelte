<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import WarningCircle from 'phosphor-svelte/lib/WarningCircle';
  import {
    WVSTBridgeWorkerClient,
    WVSTClient,
    WVST_VERSION,
    createLoopbackSharedBuffers,
    readLoopbackMetrics,
    type InstanceDescriptor,
    type PluginDescriptor
  } from '@wvst/web';
  import BridgeWorker from '@wvst/web/bridge-worker?worker';
  import { createRackAudioGraph } from './rack-demo/audio-graph';
  import StudioGuide from './rack-demo/StudioGuide.svelte';
  import ConnectionPanel from './rack-demo/ConnectionPanel.svelte';
  import StudioFooter from './rack-demo/StudioFooter.svelte';
  import { createDemoAudioFile, readAudioPeaks } from './rack-demo/audio-file';
  import EffectRack from './rack-demo/EffectRack.svelte';
  import PlayerDeck from './rack-demo/PlayerDeck.svelte';
  import { demoCopy } from './rack-demo/copy';
  import type { DemoStatus, Prerequisites, RackParameter, RackSlot } from './rack-demo/types';
  import { allPrerequisitesMet, createId, createPluginChoices, describeError, formatParameterValue,
    readPrerequisites, updateRackParameter } from './rack-demo/utils';

  export let locale: 'en' | 'zh' = 'en';

  const DEFAULT_ENDPOINT = 'ws://127.0.0.1:35876';
  const FRAMES = 128, CHANNELS = 2;
  const MAX_VISIBLE_PARAMETERS = 8;

  let mediaElement: HTMLAudioElement | undefined;
  let fileInput: HTMLInputElement | undefined;
  let client: WVSTClient | undefined;
  let workerClient: WVSTBridgeWorkerClient | undefined;
  let worker: Worker | undefined;
  let metricsTimer: number | undefined;
  let fileUrl: string | undefined;

  let endpoint = DEFAULT_ENDPOINT;
  let token = '';
  let status: DemoStatus = 'idle';
  let statusMessage = '';
  let fileName = '';
  let peaks: number[] = [];
  let waveformBusy = false;
  let connectionIssue = '';
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

  const graph = createRackAudioGraph({
    mediaElement: () => mediaElement, rack: () => rack,
    gain: () => muted ? 0 : volume, onLevels: (next) => { levels = next; }
  });

  $: t = demoCopy[locale] ?? demoCopy.en;
  $: connected = Boolean(client && workerClient) && status !== 'connecting';
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
    connectionIssue = '';
    status = 'connecting';
    statusMessage = t.connecting;
    try {
      client = await WVSTClient.connect({
        endpoint: targetEndpoint,
        clientName: 'wvst-docs-demo',
        clientVersion: WVST_VERSION,
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
      await workerClient.request('bridge.hello', {
        ...client.createHelloRequest().params,
        ...(targetToken ? { token: targetToken } : {})
      });
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
        status = 'idle';
        statusMessage = '';
        connectionIssue = describeError(error);
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
    peaks = [];
    waveformBusy = true;
    const selectedUrl = fileUrl;
    void readAudioPeaks(file).catch(() => []).then((next) => {
      if (!disposed && fileUrl === selectedUrl) { peaks = next; waveformBusy = false; }
    });
    currentTime = 0;
    duration = 0;
    isPlaying = false;
    if (mediaElement) {
      mediaElement.src = fileUrl;
      mediaElement.load();
    }
    status = connected ? 'connected' : 'idle';
    statusMessage = t.fileReady;
  }

  async function togglePlayback() {
    if (!mediaElement || !fileUrl) return;
    try {
      await graph.ensureAudioGraph();
      await graph.resume();
      if (mediaElement.paused) { await mediaElement.play(); isPlaying = true; }
      else { mediaElement.pause(); isPlaying = false; }
    } catch (error) { reportAudioError(error); }
  }

  function reportAudioError(error?: unknown) {
    isPlaying = false;
    status = 'error';
    statusMessage = error instanceof Error ? `${t.audioError} ${error.message}` : t.audioError;
  }

  function loadSample() {
    loadFile(createDemoAudioFile());
    loop = true;
    if (mediaElement) mediaElement.loop = true;
  }

  async function scanPlugins() {
    if (busy) return;
    busy = true;
    try { await loadPlugins(true); } finally { busy = false; }
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
    graph.updateOutputGain();
  }

  function toggleMute() {
    muted = !muted;
    graph.updateOutputGain();
  }

  function toggleLoop() {
    loop = !loop;
    if (mediaElement) mediaElement.loop = loop;
  }

  async function mountSelectedEffect() {
    if (!client || !workerClient || !selectedChoice || busy) return;
    busy = true;
    let instance: InstanceDescriptor | undefined;
    let node: AudioWorkletNode | undefined;
    try {
      const context = await graph.ensureAudioContext();
      await graph.ensureAudioGraph();
      instance = await client.instances.create({
        pluginId: selectedChoice.pluginId,
        ...(selectedChoice.classId ? { classId: selectedChoice.classId } : {}),
        sampleRate: Math.round(context.sampleRate),
        maxBlockFrames: FRAMES,
        inputChannels: CHANNELS,
        outputChannels: CHANNELS
      });
      instance = (await client.instances.start({ instanceId: instance.instanceId })).instance;
      const buffers = createLoopbackSharedBuffers({ frames: FRAMES, inputChannels: CHANNELS, outputChannels: CHANNELS, capacityQuanta: 4 });
      node = await graph.createRackNode(context, buffers);
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
      graph.rebuildGraph();
      status = 'connected';
      statusMessage = t.mounted;
    } catch (error) {
      node?.disconnect();
      if (instance) {
        await workerClient?.stopAudioStream(instance.streamId).catch(() => undefined);
        await destroyInstance(instance);
      }
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
    graph.rebuildGraph();
    await cleanupSlot(slot);
    rack = rack.filter((item) => item.id !== slot.id);
    graph.rebuildGraph();
    statusMessage = t.removed;
  }

  function toggleBypass(slot: RackSlot) {
    rack = rack.map((item) => item.id === slot.id ? { ...item, bypassed: !item.bypassed } : item);
    graph.rebuildGraph();
  }

  function moveSlot(index: number, direction: -1 | 1) {
    const nextIndex = index + direction;
    if (nextIndex < 0 || nextIndex >= rack.length) return;
    const next = [...rack];
    const [slot] = next.splice(index, 1);
    if (!slot) return;
    next.splice(nextIndex, 0, slot);
    rack = next;
    graph.rebuildGraph();
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
    graph.stopMeterLoop();
    mediaElement?.pause();
    isPlaying = false;
    for (const slot of [...rack]) await cleanupSlot(slot).catch(() => undefined);
    rack = [];
    plugins = [];
    scanFailures = [];
    selectedChoiceKey = '';
    await closeBridgeOnly();
    // An HTMLMediaElement can only acquire one MediaElementAudioSourceNode.
    // Keep its graph for reconnects; release it when the component is removed.
    if (!disposed) {
      graph.rebuildGraph();
      return;
    }
    await graph.close();
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

  function formatPrerequisiteMessage(value: Prerequisites): string {
    const missing = [value.secureContext ? '' : t.secureContext, value.crossOriginIsolated ? '' : t.isolation, value.sharedArrayBuffer ? '' : t.sharedBuffer].filter(Boolean);
    return `${t.blocked} ${missing.join(', ')}`;
  }

</script>

<section class="wvst-demo" aria-label="WVST live studio">
  <header class="studio-heading">
    <div><p class="studio-eyebrow">WVST / LIVE STUDIO <span>EXPERIMENTAL</span></p>
      <h1>{t.studioTitle}</h1><p class="studio-intro">{t.studioIntro}</p>
    </div>
    <a class="studio-inline-link" href={locale === 'zh' ? '/docs/zh/demo-guide' : '/docs/demo-guide'}>{t.guideLink} ↗</a>
  </header>

  <StudioGuide {t} {connected} hasAudio={Boolean(fileUrl)} hasEffect={rack.length > 0} />
  <ConnectionPanel {t} {locale} {connected} {busy} {status} {prerequisites} {connectionIssue}
    bind:endpoint bind:token onConnect={connectBridge} onDisconnect={disconnectBridge} />

  {#if status === 'error' || status === 'blocked'}
    <div class="studio-notice" role="alert"><WarningCircle size={19} /><span>{statusMessage}</span></div>
  {/if}

  <div class="studio-workspace-heading"><span>{t.sessionNote}</span><span class="studio-live-status" class:online={connected}><i></i>{connected ? t.connected : t.offline}</span></div>
  <div class="wvst-demo-layout">
    <PlayerDeck {t} bind:mediaElement bind:fileInput {fileUrl} {fileName} {isPlaying}
      {currentTime} {duration} {volume} {muted} {loop} {activeSlots} {peaks} {waveformBusy}
      onChooseFile={chooseFile} onSample={loadSample} onFileSelected={handleFileSelected} onDropFile={handleFileDrop}
      onTogglePlayback={togglePlayback} onStop={stopPlayback} onSkip={skip} onSeek={seek}
      onVolume={changeVolume} onToggleMute={toggleMute} onToggleLoop={toggleLoop}
      onTimeUpdate={updateTime} onEnded={() => (isPlaying = false)} onAudioError={() => reportAudioError()} />
    <EffectRack {t} {locale} {pluginChoices} bind:selectedChoiceKey {connected} {busy} {rack} {scanFailures}
      onMountEffect={mountSelectedEffect} onScan={scanPlugins} onMove={moveSlot} onBypass={toggleBypass}
      onRemove={removeSlot} onPreviewParameter={previewParameter} onCommitParameter={commitParameter} />
  </div>
  <StudioFooter {t} {locale} {rack} {fileName} {levels} {isPlaying} />
</section>
