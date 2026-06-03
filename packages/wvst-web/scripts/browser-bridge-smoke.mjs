import { spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(packageRoot, "../..");
const exeSuffix = process.platform === "win32" ? ".exe" : "";
const reportPath = resolve(
  repoRoot,
  process.env.WVST_BRIDGE_SMOKE_REPORT ?? "output/evidence/web-bridge-smoke.json",
);
const smokeConfig = {
  endpoint: process.env.WVST_BRIDGE_SMOKE_ENDPOINT,
  token: stringFromEnv("WVST_BRIDGE_SMOKE_TOKEN"),
  pluginPath: requiredStringFromEnv("WVST_BRIDGE_SMOKE_PLUGIN_PATH"),
  pluginId: stringFromEnv("WVST_BRIDGE_SMOKE_PLUGIN_ID"),
  classId: stringFromEnv("WVST_BRIDGE_SMOKE_CLASS_ID"),
  frames: integerFromEnv("WVST_BRIDGE_SMOKE_FRAMES", 128),
  inputChannels: integerFromEnv("WVST_BRIDGE_SMOKE_INPUT_CHANNELS", 2, {
    allowZero: true,
  }),
  outputChannels: integerFromEnv("WVST_BRIDGE_SMOKE_OUTPUT_CHANNELS", 2),
  inputBusIndex: optionalIntegerFromEnv("WVST_BRIDGE_SMOKE_INPUT_BUS_INDEX", {
    allowZero: true,
  }),
  outputBusIndex: optionalIntegerFromEnv("WVST_BRIDGE_SMOKE_OUTPUT_BUS_INDEX", {
    allowZero: true,
  }),
  capacityQuanta: integerFromEnv("WVST_BRIDGE_SMOKE_CAPACITY_QUANTA", 64),
  durationMs: integerFromEnv("WVST_BRIDGE_SMOKE_DURATION_MS", 1_000),
  pollMs: integerFromEnv("WVST_BRIDGE_SMOKE_POLL_MS", 5),
  minRoundTrips: integerFromEnv("WVST_BRIDGE_SMOKE_MIN_ROUND_TRIPS", 8),
  midiNote: integerFromEnv("WVST_BRIDGE_SMOKE_MIDI_NOTE", 60),
  requireNonSilentOutput: booleanFromEnv("WVST_BRIDGE_SMOKE_REQUIRE_NON_SILENT", false),
};

const MIME_TYPES = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".map", "application/json; charset=utf-8"],
]);

const bridge = await maybeStartBridge();
const smokeHtml = bridgeSmokeHtml({ ...smokeConfig, endpoint: bridge.endpoint });
const server = createStaticServer(smokeHtml);

await new Promise((resolve, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolve);
});

const address = server.address();
if (!address || typeof address === "string") {
  throw new Error("WVST bridge smoke server did not bind to a TCP port");
}

let browser;
try {
  browser = await chromium.launch({
    headless: process.env.WVST_BRIDGE_SMOKE_HEADLESS !== "0",
    args: ["--autoplay-policy=no-user-gesture-required"],
  });
  const page = await browser.newPage();
  const diagnostics = { console: [], pageErrors: [] };
  page.on("console", (message) => diagnostics.console.push(message.text()));
  page.on("pageerror", (error) => diagnostics.pageErrors.push(error.message));
  await page.goto(`http://127.0.0.1:${address.port}/bridge-smoke`, {
    waitUntil: "networkidle",
  });
  try {
    await page.waitForFunction(
      () => globalThis.__WVST_BRIDGE_SMOKE_RESULT__,
      undefined,
      { timeout: smokeConfig.durationMs + 12_000 },
    );
  } catch (error) {
    throw new Error(
      `${error instanceof Error ? error.message : String(error)}; diagnostics=${JSON.stringify(diagnostics)}`,
    );
  }

  const result = await page.evaluate(() => globalThis.__WVST_BRIDGE_SMOKE_RESULT__);
  await mkdir(dirname(reportPath), { recursive: true });
  await writeFile(reportPath, `${JSON.stringify(result, null, 2)}\n`);

  if (!result.ok) {
    throw new Error(result.error ?? "WVST bridge browser smoke failed without an error");
  }

  console.log(JSON.stringify({ reportPath, ...result }, null, 2));
} finally {
  await browser?.close();
  await new Promise((resolve) => server.close(resolve));
  await bridge.stop();
}

function bridgeSmokeHtml(config) {
  return `<!doctype html>
<meta charset="utf-8">
<title>WVST Bridge Smoke</title>
<script type="module">
import {
  LoopbackCounter,
  MidiEventKind,
  WVSTBridgeWorkerClient,
  WVSTClient,
  createLoopbackAudioWorkletNode,
  createLoopbackSharedBuffers,
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

  let instance;
  let node;
  let oscillator;
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
      await bridgeWorker.sendMidiEvents({
        streamId: instance.streamId,
        events: [{
          sampleOffset: 0,
          kind: MidiEventKind.NoteOn,
          channel: 0,
          data1: result.config.midiNote,
          data2: 100,
          noteId: result.config.midiNote,
        }],
      });
    }

    const metrics = await waitForMetrics(buffers, result.config);
    result.metrics = {
      ...metrics,
      endToEndRoundTripUs: percentileSnapshot(roundTripSamplesUs),
    };
    result.outputEnergy = outputEnergy(buffers);
    result.bridgeMetrics = await client.metrics();
    result.runtimeSnapshot = await client.instances.runtimeSnapshot({
      instanceId: instance.instanceId,
    });
    result.ok =
      result.metrics.inputSequence >= result.config.minRoundTrips &&
      result.metrics.outputConsumedSequence >= result.config.minRoundTrips &&
      result.metrics.endToEndRoundTripUs.count >= result.config.minRoundTrips &&
      result.metrics.transportFailures === 0 &&
      (!result.config.requireNonSilentOutput || result.outputEnergy.peak > 0);
    if (!result.ok && result.config.requireNonSilentOutput && result.outputEnergy.peak === 0) {
      result.error = "WVST bridge smoke produced only silent output";
    }
  } finally {
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

function percentile(sortedValues, percentileValue) {
  const index = Math.min(
    sortedValues.length - 1,
    Math.max(0, Math.ceil(sortedValues.length * percentileValue) - 1),
  );
  return sortedValues[index];
}
</script>`;
}

function createStaticServer(smokeHtml) {
  return createServer(async (request, response) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1");
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");

    try {
      if (url.pathname === "/" || url.pathname === "/bridge-smoke") {
        response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
        response.end(smokeHtml);
        return;
      }

      if (!url.pathname.startsWith("/dist/")) {
        response.writeHead(404);
        response.end("not found");
        return;
      }

      const filePath = resolve(packageRoot, `.${url.pathname}`);
      if (!filePath.startsWith(join(packageRoot, "dist"))) {
        response.writeHead(403);
        response.end("forbidden");
        return;
      }

      const body = await readFile(filePath);
      response.writeHead(200, {
        "content-type": MIME_TYPES.get(extname(filePath)) ?? "application/octet-stream",
      });
      response.end(body);
    } catch (error) {
      response.writeHead(500, { "content-type": "text/plain; charset=utf-8" });
      response.end(error instanceof Error ? error.message : String(error));
    }
  });
}

async function maybeStartBridge() {
  if (smokeConfig.endpoint) {
    return { endpoint: smokeConfig.endpoint, stop: async () => undefined };
  }

  const bridgeServer = resolve(
    repoRoot,
    process.env.WVST_BRIDGE_SERVER ?? `target/debug/wvst-bridge-server${exeSuffix}`,
  );
  const hostWorker = resolve(
    repoRoot,
    process.env.WVST_HOST_WORKER ?? `target/debug/wvst-host-worker${exeSuffix}`,
  );
  const child = spawn(bridgeServer, ["serve"], {
    env: {
      ...process.env,
      WVST_BIND_ADDR: process.env.WVST_BIND_ADDR ?? "127.0.0.1:0",
      WVST_HOST_WORKER: hostWorker,
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const endpoint = await waitForBridgeEndpoint(child);

  return {
    endpoint,
    stop: () => stopChild(child),
  };
}

function waitForBridgeEndpoint(child) {
  let stderr = "";
  let stdout = "";
  return new Promise((resolvePromise, reject) => {
    const timeout = setTimeout(() => {
      cleanup();
      reject(new Error(`WVST bridge server did not report an endpoint; stderr=${stderr}`));
    }, integerFromEnv("WVST_BRIDGE_SMOKE_START_TIMEOUT_MS", 5_000));

    const cleanup = () => {
      clearTimeout(timeout);
      child.stderr.off("data", onStderr);
      child.stdout.off("data", onStdout);
      child.off("exit", onExit);
      child.off("error", onError);
    };
    const onStderr = (chunk) => {
      stderr += chunk.toString("utf8");
      const match = stderr.match(/listening on (ws:\/\/[^\s]+)/);
      if (match) {
        cleanup();
        resolvePromise(match[1]);
      }
    };
    const onStdout = (chunk) => {
      stdout += chunk.toString("utf8");
    };
    const onExit = (code, signal) => {
      cleanup();
      reject(
        new Error(
          `WVST bridge server exited before startup: code=${code} signal=${signal} stdout=${stdout} stderr=${stderr}`,
        ),
      );
    };
    const onError = (error) => {
      cleanup();
      reject(error);
    };

    child.stderr.on("data", onStderr);
    child.stdout.on("data", onStdout);
    child.once("exit", onExit);
    child.once("error", onError);
  });
}

function stopChild(child) {
  if (child.exitCode !== null || child.signalCode !== null) {
    return Promise.resolve();
  }
  child.kill();
  return new Promise((resolvePromise) => {
    const timeout = setTimeout(() => {
      child.kill("SIGKILL");
      resolvePromise();
    }, 2_000);
    child.once("exit", () => {
      clearTimeout(timeout);
      resolvePromise();
    });
  });
}

function requiredStringFromEnv(name) {
  const value = stringFromEnv(name);
  if (!value) {
    throw new Error(`${name} is required`);
  }
  return value;
}

function stringFromEnv(name) {
  const raw = process.env[name];
  return raw === undefined || raw.length === 0 ? undefined : raw;
}

function integerFromEnv(name, fallback, options = {}) {
  const raw = process.env[name];
  if (raw === undefined || raw.length === 0) {
    return fallback;
  }
  const parsed = Number.parseInt(raw, 10);
  const valid = Number.isFinite(parsed) && (options.allowZero ? parsed >= 0 : parsed > 0);
  if (!valid) {
    const label = options.allowZero ? "a non-negative integer" : "a positive integer";
    throw new Error(`${name} must be ${label}`);
  }
  return parsed;
}

function optionalIntegerFromEnv(name, options = {}) {
  const raw = process.env[name];
  if (raw === undefined || raw.length === 0) {
    return undefined;
  }
  return integerFromEnv(name, 0, options);
}

function booleanFromEnv(name, fallback) {
  const raw = process.env[name];
  if (raw === undefined || raw.length === 0) {
    return fallback;
  }
  return raw !== "0" && !raw.toLowerCase().startsWith("false");
}
