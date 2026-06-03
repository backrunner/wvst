import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const packageRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(packageRoot, "../..");
const reportPath = resolve(
  repoRoot,
  process.env.WVST_BROWSER_SMOKE_REPORT ?? "output/playwright/web-audio-smoke.json",
);
const smokeConfig = {
  frames: integerFromEnv("WVST_BROWSER_SMOKE_FRAMES", 128),
  outputChannels: integerFromEnv("WVST_BROWSER_SMOKE_OUTPUT_CHANNELS", 1),
  capacityQuanta: integerFromEnv("WVST_BROWSER_SMOKE_CAPACITY_QUANTA", 64),
  durationMs: integerFromEnv("WVST_BROWSER_SMOKE_DURATION_MS", 750),
  pollMs: integerFromEnv("WVST_BROWSER_SMOKE_POLL_MS", 5),
  minRoundTrips: integerFromEnv("WVST_BROWSER_SMOKE_MIN_ROUND_TRIPS", 8),
};

const MIME_TYPES = new Map([
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".map", "application/json; charset=utf-8"],
]);

const smokeHtml = `<!doctype html>
<meta charset="utf-8">
<title>WVST WebAudio Smoke</title>
<script type="module">
import {
  LoopbackCounter,
  createLoopbackAudioWorkletNode,
  createLoopbackSharedBuffers,
  readLoopbackMetrics,
} from "/dist/esm/index.js";

const result = {
  ok: false,
  crossOriginIsolated: globalThis.crossOriginIsolated === true,
  sharedArrayBuffer: typeof globalThis.SharedArrayBuffer === "function",
  audioWorklet: false,
  sampleRate: 0,
  config: ${JSON.stringify(smokeConfig)},
  metrics: undefined,
};
const outputWriteTimes = new Map();
const roundTripSamplesUs = [];
let lastRecordedOutputSequence = 0;

async function waitForMetrics(buffers) {
  const deadline = performance.now() + result.config.durationMs;
  let metrics = readLoopbackMetrics(buffers);
  while (performance.now() < deadline) {
    const inputSequence = Atomics.load(buffers.counters, LoopbackCounter.InputSequence);
    Atomics.store(buffers.counters, LoopbackCounter.InputConsumedSequence, inputSequence);
    keepOutputAhead(buffers);

    metrics = readLoopbackMetrics(buffers);
    recordConsumedOutputs(metrics.outputConsumedSequence);
    await new Promise((resolve) => setTimeout(resolve, result.config.pollMs));
  }
  metrics = readLoopbackMetrics(buffers);
  recordConsumedOutputs(metrics.outputConsumedSequence);
  if (
    metrics.inputSequence < result.config.minRoundTrips ||
    metrics.outputConsumedSequence < result.config.minRoundTrips
  ) {
    throw new Error("insufficient AudioWorklet loopback samples");
  }
  return metrics;
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

function keepOutputAhead(buffers) {
  let outputSequence = Atomics.load(buffers.counters, LoopbackCounter.OutputSequence);
  const consumedSequence = Atomics.load(
    buffers.counters,
    LoopbackCounter.OutputConsumedSequence,
  );
  while (outputSequence - consumedSequence < buffers.capacityQuanta) {
    outputSequence += 1;
    writeOutputQuantum(buffers, outputSequence);
  }
}

function writeOutputQuantum(buffers, sequence) {
  const offset = ((sequence - 1) % buffers.capacityQuanta) * buffers.outputSamplesPerQuantum;
  buffers.outputSamples.fill(0.125, offset, offset + buffers.outputSamplesPerQuantum);
  outputWriteTimes.set(sequence, performance.now());
  Atomics.store(buffers.counters, LoopbackCounter.OutputSequence, sequence);
}

function recordConsumedOutputs(outputConsumedSequence) {
  const observedAt = performance.now();
  for (
    let sequence = lastRecordedOutputSequence + 1;
    sequence <= outputConsumedSequence;
    sequence += 1
  ) {
    const writtenAt = outputWriteTimes.get(sequence);
    if (writtenAt !== undefined) {
      roundTripSamplesUs.push(Math.max(0, Math.round((observedAt - writtenAt) * 1_000)));
      outputWriteTimes.delete(sequence);
    }
  }
  lastRecordedOutputSequence = Math.max(lastRecordedOutputSequence, outputConsumedSequence);
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
  result.sampleRate = context.sampleRate;

  const buffers = createLoopbackSharedBuffers({
    frames: result.config.frames,
    inputChannels: 0,
    outputChannels: result.config.outputChannels,
    capacityQuanta: result.config.capacityQuanta,
  });
  const node = await createLoopbackAudioWorkletNode(context, {
    processorUrl: "/dist/esm/audio/loopback-processor.js",
    inputChannels: 0,
    outputChannels: result.config.outputChannels,
    buffers,
  });
  const gain = context.createGain();
  gain.gain.value = 0;
  node.connect(gain).connect(context.destination);
  keepOutputAhead(buffers);

  await withTimeout(context.resume(), 1_000, "AudioContext resume");
  const metrics = await waitForMetrics(buffers);
  result.metrics = {
    ...metrics,
    endToEndRoundTripUs: percentileSnapshot(roundTripSamplesUs),
  };
  result.ok =
    result.metrics.inputSequence >= result.config.minRoundTrips &&
    result.metrics.outputConsumedSequence >= result.config.minRoundTrips &&
    result.metrics.endToEndRoundTripUs.count >= result.config.minRoundTrips;
  await context.close();
} catch (error) {
  result.error = error instanceof Error ? error.message : String(error);
}

globalThis.__WVST_BROWSER_SMOKE_RESULT__ = result;
</script>`;

const server = createServer(async (request, response) => {
  const url = new URL(request.url ?? "/", "http://127.0.0.1");
  response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
  response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");

  try {
    if (url.pathname === "/" || url.pathname === "/smoke") {
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

await new Promise((resolve, reject) => {
  server.once("error", reject);
  server.listen(0, "127.0.0.1", resolve);
});

const address = server.address();
if (!address || typeof address === "string") {
  throw new Error("WVST browser smoke server did not bind to a TCP port");
}

let browser;
try {
  browser = await chromium.launch({
    headless: process.env.WVST_BROWSER_SMOKE_HEADLESS !== "0",
    args: ["--autoplay-policy=no-user-gesture-required"],
  });
  const page = await browser.newPage();
  const diagnostics = { console: [], pageErrors: [] };
  page.on("console", (message) => {
    diagnostics.console.push(message.text());
  });
  page.on("pageerror", (error) => {
    diagnostics.pageErrors.push(error.message);
  });
  await page.goto(`http://127.0.0.1:${address.port}/smoke`, {
    waitUntil: "networkidle",
  });
  try {
    await page.waitForFunction(
      () => globalThis.__WVST_BROWSER_SMOKE_RESULT__,
      undefined,
      { timeout: smokeConfig.durationMs + 6_000 },
    );
  } catch (error) {
    throw new Error(
      `${error instanceof Error ? error.message : String(error)}; diagnostics=${JSON.stringify(diagnostics)}`,
    );
  }
  const result = await page.evaluate(() => globalThis.__WVST_BROWSER_SMOKE_RESULT__);
  await mkdir(dirname(reportPath), { recursive: true });
  await writeFile(reportPath, `${JSON.stringify(result, null, 2)}\n`);

  if (!result.ok) {
    throw new Error(result.error ?? "WVST browser smoke failed without an error");
  }

  console.log(JSON.stringify({ reportPath, ...result }, null, 2));
} finally {
  await browser?.close();
  await new Promise((resolve) => server.close(resolve));
}

function integerFromEnv(name, fallback) {
  const raw = process.env[name];
  if (raw === undefined || raw.length === 0) {
    return fallback;
  }
  const parsed = Number.parseInt(raw, 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive integer`);
  }
  return parsed;
}
