import { spawn } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { bridgeSmokeHtml } from "./browser-bridge-smoke-page.mjs";

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
  verifyMidiRelease: booleanFromEnv("WVST_BRIDGE_SMOKE_VERIFY_MIDI_RELEASE", false),
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
      { timeout: smokeConfig.durationMs + 190_000 },
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
      const match = stderr.match(/listening on (ws:\/\/[^\s]+)\r?\n/);
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
