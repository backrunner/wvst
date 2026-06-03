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
  metrics: undefined,
};
const outputWriteTimes = new Map();
const roundTripSamplesUs = [];
let lastRecordedOutputSequence = 0;

async function waitForMetrics(buffers) {
  const deadline = performance.now() + 4_000;
  while (performance.now() < deadline) {
    const inputSequence = Atomics.load(buffers.counters, LoopbackCounter.InputSequence);
    Atomics.store(buffers.counters, LoopbackCounter.InputConsumedSequence, inputSequence);
    keepOutputAhead(buffers);

    const metrics = readLoopbackMetrics(buffers);
    recordConsumedOutputs(metrics.outputConsumedSequence);
    if (metrics.inputSequence >= 8 && metrics.outputConsumedSequence >= 8) {
      return metrics;
    }
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw new Error("timed out waiting for AudioWorklet loopback metrics");
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
    frames: 128,
    inputChannels: 0,
    outputChannels: 1,
    capacityQuanta: 64,
  });
  const node = await createLoopbackAudioWorkletNode(context, {
    processorUrl: "/dist/esm/audio/loopback-processor.js",
    inputChannels: 0,
    outputChannels: 1,
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
    result.metrics.inputSequence >= 8 &&
    result.metrics.outputConsumedSequence >= 8 &&
    result.metrics.endToEndRoundTripUs.count >= 8;
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
      { timeout: 6_000 },
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
