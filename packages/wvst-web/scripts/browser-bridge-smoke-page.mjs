export function bridgeSmokeHtml(config) {
  return `<!doctype html>
<meta charset="utf-8">
<title>WVST Bridge Smoke</title>
<script type="module">
import {
  LoopbackCounter,
  WVSTBridgeWorkerClient,
  WVSTClient,
  createLoopbackAudioWorkletNode,
  createLoopbackSharedBuffers,
  createWVSTWebMidiAdapter,
  readLoopbackMetrics,
} from "/dist/esm/index.js";

const result = {
  ok: false,
  mode: "bridge-vst3",
  crossOriginIsolated: globalThis.crossOriginIsolated === true,
  sharedArrayBuffer: typeof globalThis.SharedArrayBuffer === "function",
  audioWorklet: false,
  sampleRate: 0,
  config: ${JSON.stringify(config)},
  plugin: undefined,
  instance: undefined,
  bridgeMetrics: undefined,
  runtimeSnapshot: undefined,
  outputEnergy: undefined,
  metrics: undefined,
  midi: undefined,
};
const inputTimes = new Map();
const roundTripSamplesUs = [];
let lastInputSequence = 0;
let lastOutputConsumedSequence = 0;

try {
  if (!result.crossOriginIsolated || !result.sharedArrayBuffer) {
    throw new Error("SharedArrayBuffer requires cross-origin isolation");
  }

  const AudioContextCtor = globalThis.AudioContext || globalThis.webkitAudioContext;
  if (!AudioContextCtor) {
    throw new Error("AudioContext is not available");
  }

  const context = new AudioContextCtor({ latencyHint: "interactive" });
  result.audioWorklet = Boolean(context.audioWorklet);
  result.sampleRate = Math.round(context.sampleRate);

  const client = await WVSTClient.connect({
    endpoint: result.config.endpoint,
    token: result.config.token,
    requireLowLatency: true,
  });
  const bridgeWorker = new WVSTBridgeWorkerClient({
    worker: new Worker("/dist/esm/audio/bridge-worker.js", { type: "module" }),
  });
  await bridgeWorker.connect(result.config.endpoint);
  await bridgeWorker.request("bridge.hello", {
    ...client.createHelloRequest().params,
    token: result.config.token,
  });

  let instance;
  let node;
  let oscillator;
  let midi;
  try {
    const plugin = await selectPlugin(client, result.config);
    const classId = await selectClassId(client, plugin, result.config);
    result.plugin = {
      pluginId: plugin.pluginId,
      name: plugin.name,
      vendor: plugin.vendor,
      path: plugin.path,
      classId,
    };

    const created = await client.instances.create({
      pluginId: plugin.pluginId,
      classId,
      sampleRate: result.sampleRate,
      maxBlockFrames: result.config.frames,
      inputChannels: result.config.inputChannels,
      outputChannels: result.config.outputChannels,
      inputBusIndex: result.config.inputBusIndex,
      outputBusIndex: result.config.outputBusIndex,
    });
    instance = created;
    await client.instances.start({ instanceId: created.instanceId });
    instance = await client.instances.openStream({ instanceId: created.instanceId });
    result.instance = instance;

    const buffers = createLoopbackSharedBuffers({
      frames: result.config.frames,
      inputChannels: result.config.inputChannels,
      outputChannels: result.config.outputChannels,
      capacityQuanta: result.config.capacityQuanta,
    });
    node = await createLoopbackAudioWorkletNode(context, {
      processorUrl: "/dist/esm/audio/loopback-processor.js",
      inputChannels: result.config.inputChannels,
      outputChannels: result.config.outputChannels,
      buffers,
    });
    if (result.config.inputChannels > 0) {
      oscillator = context.createOscillator();
      const inputGain = context.createGain();
      inputGain.gain.value = 0.05;
      oscillator.frequency.value = 220;
      oscillator.connect(inputGain).connect(node);
      oscillator.start();
    }
    const outputGain = context.createGain();
    outputGain.gain.value = 0;
    node.connect(outputGain).connect(context.destination);

    await bridgeWorker.startAudioStream({
      streamId: instance.streamId,
      sampleRate: instance.sampleRate,
      frames: result.config.frames,
      inputChannels: instance.inputChannels,
      outputChannels: instance.outputChannels,
      buffers,
    });
    await withTimeout(context.resume(), 1_000, "AudioContext resume");
    if (result.config.inputChannels === 0) {
      midi = await createMidiInput(bridgeWorker, instance, buffers);
      await midi.noteOn(result.config.midiNote);
    }

    const metrics = await waitForMetrics(buffers, result.config);
    result.metrics = {
      ...metrics,
      endToEndRoundTripUs: roundTripEstimate(metrics),
    };
    result.outputEnergy = outputEnergy(buffers);
    if (midi) {
      await midi.stop();
      if (result.config.verifyMidiRelease) {
        await verifyMidiRelease(buffers, result.config);
      }
    }
    result.bridgeMetrics = await client.metrics();
    result.runtimeSnapshot = await client.instances.runtimeSnapshot({
      instanceId: instance.instanceId,
    });
    result.ok =
      result.metrics.inputSequence >= result.config.minRoundTrips &&
      result.metrics.outputConsumedSequence >= result.config.minRoundTrips &&
      result.metrics.transportFailures === 0 &&
      (!result.midi || (result.midi.errors.length === 0 && result.midi.detached)) &&
      (!result.config.requireNonSilentOutput || result.outputEnergy.peak > 0);
    if (!result.ok && result.config.requireNonSilentOutput && result.outputEnergy.peak === 0) {
      result.error = "WVST bridge smoke produced only silent output";
    }
  } finally {
    await midi?.stop();
    oscillator?.stop();
    node?.disconnect();
    if (instance) {
      try {
        await bridgeWorker.stopAudioStream(instance.streamId);
      } catch {}
      try {
        await client.instances.closeStream({ instanceId: instance.instanceId });
      } catch {}
      try {
        await client.instances.stop({ instanceId: instance.instanceId });
      } catch {}
      try {
        await client.instances.destroy({ instanceId: instance.instanceId });
      } catch {}
    }
    await bridgeWorker.close().catch(() => undefined);
    client.close();
    await context.close();
  }
} catch (error) {
  result.error = error instanceof Error ? error.message : String(error);
}

globalThis.__WVST_BRIDGE_SMOKE_RESULT__ = result;

async function selectPlugin(client, config) {
  const report = await client.plugins.scan({ paths: [config.pluginPath] });
  const plugin =
    report.plugins.find((candidate) => candidate.pluginId === config.pluginId) ??
    report.plugins.find((candidate) => candidate.path === config.pluginPath) ??
    report.plugins[0];
  if (!plugin) {
    throw new Error("WVST bridge smoke did not find a VST3 plugin");
  }
  return plugin;
}

async function createMidiInput(bridgeWorker, instance, buffers) {
  const listeners = new Set();
  const input = {
    id: "smoke-synthetic-input",
    state: "connected",
    addEventListener: (_type, listener) => listeners.add(listener),
    removeEventListener: (_type, listener) => listeners.delete(listener),
  };
  result.midi = { source: "synthetic-web-midi-input", events: [], errors: [], detached: false };
  let lastSend = Promise.resolve();
  const adapter = await createWVSTWebMidiAdapter({
    buffers,
    sendMidiEvents: (events) => {
      result.midi.events.push(...events);
      lastSend = bridgeWorker.sendMidiEvents({ streamId: instance.streamId, events });
      return lastSend;
    },
  }, {
    midiAccess: { inputs: new Map([[input.id, input]]) },
    onError: (error) => result.midi.errors.push(error.message),
  });
  return {
    noteOn: async (note) => {
      if (listeners.size !== 1) throw new Error("Web MIDI input was not bound exactly once");
      for (const listener of listeners) listener({ data: new Uint8Array([0x90, note, 100]) });
      await lastSend;
    },
    stop: async () => {
      await adapter.stop();
      result.midi.detached = listeners.size === 0;
    },
  };
}

async function verifyMidiRelease(buffers, config) {
  // Opt-in for fixtures with no release tail. Wait until every output ring slot
  // has been replaced after the adapter enqueues its tracked Note Off.
  const baseline = readLoopbackMetrics(buffers).outputSequence;
  const deadline = performance.now() + 2_000;
  while (performance.now() < deadline) {
    const metrics = readLoopbackMetrics(buffers);
    if (metrics.transportFailures || metrics.droppedMidiEvents || result.midi.errors.length) {
      throw new Error("Audio/MIDI failure during Web MIDI release verification");
    }
    const energy = outputEnergy(buffers);
    if (metrics.outputSequence > baseline + config.capacityQuanta + 2 && energy.peak === 0) {
      result.midi.releaseOutputEnergy = energy;
      result.midi.releaseVerified = true;
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, config.pollMs));
  }
  result.midi.releaseOutputEnergy = outputEnergy(buffers);
  result.midi.releaseVerified = false;
  throw new Error("Web MIDI stop did not produce silent output within 2 seconds");
}

async function selectClassId(client, plugin, config) {
  if (config.classId) {
    return config.classId;
  }
  const fromScanner = plugin.classes.find((candidate) => candidate.classId)?.classId;
  if (fromScanner) {
    return fromScanner;
  }
  const factory = await client.plugins.factoryInfo({ path: plugin.path });
  const fromFactory = factory.classes.find((candidate) => candidate.classId)?.classId;
  if (!fromFactory) {
    throw new Error("WVST bridge smoke could not select a VST3 class id");
  }
  return fromFactory;
}

async function waitForMetrics(buffers, config) {
  const deadline = performance.now() + config.durationMs;
  let metrics = readLoopbackMetrics(buffers);
  while (performance.now() < deadline) {
    observeSequences(buffers);
    metrics = readLoopbackMetrics(buffers);
    await new Promise((resolve) => setTimeout(resolve, config.pollMs));
  }
  observeSequences(buffers);
  metrics = readLoopbackMetrics(buffers);
  // Preserve overload diagnostics even when the acceptance threshold fails.
  result.metrics = { ...metrics, endToEndRoundTripUs: roundTripEstimate(metrics) };
  result.outputEnergy = outputEnergy(buffers);
  if (
    metrics.inputSequence < config.minRoundTrips ||
    metrics.outputConsumedSequence < config.minRoundTrips
  ) {
    throw new Error("insufficient Bridge/VST/WebAudio round-trip samples");
  }
  return metrics;
}

function observeSequences(buffers) {
  const observedAt = performance.now();
  const inputSequence = Atomics.load(buffers.counters, LoopbackCounter.InputSequence);
  for (let sequence = lastInputSequence + 1; sequence <= inputSequence; sequence += 1) {
    inputTimes.set(sequence, observedAt);
  }
  lastInputSequence = Math.max(lastInputSequence, inputSequence);

  const outputConsumedSequence = Atomics.load(
    buffers.counters,
    LoopbackCounter.OutputConsumedSequence,
  );
  for (
    let sequence = lastOutputConsumedSequence + 1;
    sequence <= outputConsumedSequence;
    sequence += 1
  ) {
    const inputAt = inputTimes.get(sequence);
    if (inputAt !== undefined) {
      roundTripSamplesUs.push(Math.max(0, Math.round((observedAt - inputAt) * 1_000)));
      inputTimes.delete(sequence);
    }
  }
  lastOutputConsumedSequence = Math.max(lastOutputConsumedSequence, outputConsumedSequence);
}

function outputEnergy(buffers) {
  let peak = 0;
  let sumSquares = 0;
  for (const sample of buffers.outputSamples) {
    const abs = Math.abs(sample);
    peak = Math.max(peak, abs);
    sumSquares += sample * sample;
  }
  return {
    peak,
    rms: buffers.outputSamples.length === 0
      ? 0
      : Math.sqrt(sumSquares / buffers.outputSamples.length),
  };
}

async function withTimeout(promise, timeoutMs, label) {
  let timeoutId;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timeoutId = setTimeout(() => reject(new Error(label + " timed out")), timeoutMs);
      }),
    ]);
  } finally {
    clearTimeout(timeoutId);
  }
}

function percentileSnapshot(values) {
  if (values.length === 0) {
    return { count: 0 };
  }
  const sorted = [...values].sort((left, right) => left - right);
  return {
    count: sorted.length,
    p50: percentile(sorted, 0.5),
    p95: percentile(sorted, 0.95),
    p99: percentile(sorted, 0.99),
  };
}

function roundTripEstimate(metrics) {
  if (metrics.droppedInputQuanta || metrics.droppedOutputQuanta || metrics.transportFailures) {
    return { valid: false, count: 0, reason: "Dropped or failed blocks invalidate ordinal input/output pairing" };
  }
  return {
    valid: true,
    method: "Main-thread polling of ordinal input/output counters; estimate only",
    ...percentileSnapshot(roundTripSamplesUs),
  };
}

function percentile(sortedValues, percentileValue) {
  const index = Math.min(
    sortedValues.length - 1,
    Math.max(0, Math.ceil(sortedValues.length * percentileValue) - 1),
  );
  return sortedValues[index];
}
</script>`;
}
