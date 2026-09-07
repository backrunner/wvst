import { configureLoopbackAudioWorkletNode, type LoopbackSharedBuffers } from '@wvst/web';
import loopbackProcessorUrl from '@wvst/web/loopback-processor?url';
import type { RackSlot } from './types';
import { safeDisconnect } from './utils';

interface GraphOptions {
  mediaElement: () => HTMLAudioElement | undefined;
  rack: () => RackSlot[];
  gain: () => number;
  onLevels: (levels: [number, number]) => void;
}

// Main-thread graph management only; no work is added to AudioWorklet.process().
export function createRackAudioGraph(options: GraphOptions) {
  const CHANNELS = 2;
  let audioContext: AudioContext | undefined;
  let sourceNode: MediaElementAudioSourceNode | undefined;
  let outputGain: GainNode | undefined;
  let splitter: ChannelSplitterNode | undefined;
  let merger: ChannelMergerNode | undefined;
  let analysers: AnalyserNode[] = [];
  let analyserData: Uint8Array<ArrayBuffer>[] = [];
  let workletModule: Promise<void> | undefined;
  let meterFrame: number | undefined;
  async function ensureAudioContext(): Promise<AudioContext> {
    if (audioContext) return audioContext;
    const contextCtor = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!contextCtor) throw new Error('AudioContext is not available');
    audioContext = new contextCtor();
    return audioContext;
  }

  async function ensureAudioGraph() {
    const context = await ensureAudioContext();
    const mediaElement = options.mediaElement();
    if (!mediaElement) throw new Error('Audio element is not ready');
    sourceNode ??= context.createMediaElementSource(mediaElement);
    if (!outputGain) {
      outputGain = context.createGain();
      outputGain.channelCount = CHANNELS;
      outputGain.channelCountMode = 'explicit';
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
    }
    startMeterLoop();
    rebuildGraph();
  }

  function rebuildGraph() {
    if (!sourceNode || !outputGain) return;
    safeDisconnect(sourceNode);
    options.rack().forEach((slot) => safeDisconnect(slot.node));
    let tail: AudioNode = sourceNode;
    for (const slot of options.rack()) {
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

  function startMeterLoop() {
    stopMeterLoop();
    const tick = () => {
      options.onLevels([0, 1].map((index) => readLevel(analysers[index], analyserData[index])) as [number, number]);
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


  function updateOutputGain() {
    if (!outputGain || !audioContext) return;
    outputGain.gain.setTargetAtTime(options.gain(), audioContext.currentTime, .012);
  }

  async function close() {
    stopMeterLoop();
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
  }

  return { ensureAudioContext, ensureAudioGraph, rebuildGraph, createRackNode, updateOutputGain,
    stopMeterLoop, close, resume: async () => { await audioContext?.resume(); } };
}
