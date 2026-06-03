import {
  createWVSTAudioDeviceSession,
  createWVSTVirtualKeyboard,
  createWVSTWebMidiAdapter,
  handleWVSTVirtualKeyboardEvent,
  type InstanceDescriptor,
  type PluginDescriptor,
  type WVSTAudioDeviceSession,
  type WVSTVirtualKeyboard,
  type WVSTWebMidiAdapter,
} from "@wvst/web";
import {
  element,
  fillSelect,
  optionValue,
  readInteger,
  readOptionalInteger,
  readOptionalText,
  readText,
  renderMetrics,
  setButton,
  setStatus,
  showJson,
} from "../shared/ui.js";
import {
  classChoices,
  connectBridge,
  destroyInstance,
  deviceLabel,
  listDevices,
  pluginLabel,
  processorUrl,
  stopBridge,
  type BridgeClients,
  type PluginClassChoice,
} from "../shared/wvst.js";

const NOTES = [60, 62, 64, 65, 67, 69, 71, 72];
let clients: BridgeClients | undefined;
let plugins: PluginDescriptor[] = [];
let classes: PluginClassChoice[] = [];
let context: AudioContext | undefined;
let instance: InstanceDescriptor | undefined;
let session: WVSTAudioDeviceSession | undefined;
let keyboard: WVSTVirtualKeyboard | undefined;
let webMidi: WVSTWebMidiAdapter | undefined;
let metricsTimer: number | undefined;

element("connect").addEventListener("click", () => run(connect));
element("rescan").addEventListener("click", () => run(() => scan(true)));
element("plugin").addEventListener("change", () => run(refreshClasses));
element("mount").addEventListener("click", () => run(mount));
element("web-midi").addEventListener("click", () => run(connectWebMidi));
element("stop").addEventListener("click", () => run(stop));
window.addEventListener("keydown", (event) => bindKeyboard(event));
window.addEventListener("keyup", (event) => bindKeyboard(event));
window.addEventListener("pagehide", () => void stop());
renderKeyboard();

function run(task: () => Promise<void>): void {
  void task().catch((error: unknown) => {
    setStatus(error instanceof Error ? error.message : String(error), "error");
  });
}

async function connect(): Promise<void> {
  await stop();
  setStatus("Connecting");
  clients = await connectBridge(readText("endpoint"), readOptionalText("token"));
  setButton("rescan", true);
  await populateDevices();
  await scan(false);
  setStatus("Connected", "ok");
}

async function scan(rescan: boolean): Promise<void> {
  if (!clients) {
    return;
  }
  setStatus(rescan ? "Scanning" : "Loading plugins");
  const report = await clients.client.plugins.list({ rescan });
  plugins = report.plugins;
  fillSelect(
    "plugin",
    plugins.map((plugin, index) => ({
      value: String(index),
      label: pluginLabel(plugin),
    })),
    "Plugin",
  );
  await refreshClasses();
  setStatus(`Plugins: ${plugins.length}`, "ok");
}

async function refreshClasses(): Promise<void> {
  const selected = selectedPlugin();
  if (!clients || !selected) {
    fillSelect("class", [], "Class");
    setButton("mount", false);
    return;
  }
  classes = await classChoices(clients.client, selected);
  fillSelect(
    "class",
    classes.map((choice) => ({ value: choice.classId, label: choice.label })),
    "Class",
  );
  setButton("mount", classes.length > 0);
}

async function mount(): Promise<void> {
  if (!clients) {
    throw new Error("Bridge is not connected");
  }
  const plugin = selectedPlugin();
  const classId = optionValue("class");
  if (!plugin || !classId) {
    throw new Error("Plugin class is required");
  }

  await stopMounted();
  context = new AudioContext();
  await context.resume();
  const created = await clients.client.instances.create({
    pluginId: plugin.pluginId,
    classId,
    sampleRate: Math.round(context.sampleRate),
    maxBlockFrames: readInteger("frames", 128),
    inputChannels: 0,
    outputChannels: readInteger("outputs", 2),
    outputBusIndex: readOptionalInteger("output-bus"),
  });
  await clients.client.instances.start({ instanceId: created.instanceId });
  instance = await clients.client.instances.openStream({ instanceId: created.instanceId });
  session = await createWVSTAudioDeviceSession({
    context,
    bridgeWorker: clients.worker,
    instance,
    processorUrl: processorUrl(),
    frames: instance.maxBlockFrames,
    outputDeviceId: optionValue("output-device"),
    startOutput: true,
    autoRestartAudioStream: true,
  });
  keyboard = createWVSTVirtualKeyboard({
    buffers: session.buffers,
    sendMidiEvents: (events) => session?.sendMidiEvents(events) ?? Promise.resolve(),
  });
  showJson("runtime", await clients.client.instances.runtimeSnapshot({ instanceId: instance.instanceId }));
  startMetrics();
  setButton("web-midi", true);
  setButton("stop", true);
  setStatus("Mounted", "ok");
}

async function connectWebMidi(): Promise<void> {
  if (!session) {
    throw new Error("WVST instrument session is not mounted");
  }
  webMidi?.stop();
  webMidi = await createWVSTWebMidiAdapter({
    buffers: session.buffers,
    sendMidiEvents: (events) => session?.sendMidiEvents(events) ?? Promise.resolve(),
  });
  setStatus("Web MIDI connected", "ok");
}

function renderKeyboard(): void {
  const container = element<HTMLDivElement>("keyboard");
  container.innerHTML = "";
  for (const note of NOTES) {
    const key = document.createElement("button");
    key.textContent = noteName(note);
    key.addEventListener("pointerdown", () => void keyboard?.noteOn(note, 0.85));
    key.addEventListener("pointerup", () => void keyboard?.noteOff(note));
    key.addEventListener("pointerleave", () => void keyboard?.noteOff(note));
    container.append(key);
  }
}

function bindKeyboard(event: KeyboardEvent): void {
  if (!keyboard || !handleWVSTVirtualKeyboardEvent(keyboard, event)) {
    return;
  }
  event.preventDefault();
}

async function populateDevices(): Promise<void> {
  const devices = await listDevices();
  fillSelect(
    "output-device",
    devices.outputs.map((device, index) => ({
      value: device.deviceId,
      label: deviceLabel(device, `Output ${index + 1}`),
    })),
    "Default output",
  );
}

async function stop(): Promise<void> {
  await stopMounted();
  await stopBridge(clients);
  clients = undefined;
  setButton("rescan", false);
  setButton("mount", false);
  setStatus("Idle");
}

async function stopMounted(): Promise<void> {
  window.clearInterval(metricsTimer);
  metricsTimer = undefined;
  webMidi?.stop();
  webMidi = undefined;
  await keyboard?.allNotesOff();
  keyboard = undefined;
  await session?.stop();
  session = undefined;
  await destroyInstance(clients?.client, instance);
  instance = undefined;
  context?.close();
  context = undefined;
  setButton("web-midi", false);
  setButton("stop", false);
}

function startMetrics(): void {
  window.clearInterval(metricsTimer);
  metricsTimer = window.setInterval(() => {
    if (session) {
      renderMetrics("metrics", session.getMetrics() as unknown as Record<string, unknown>);
    }
  }, 500);
}

function selectedPlugin(): PluginDescriptor | undefined {
  const index = optionValue("plugin");
  return index === undefined ? undefined : plugins[Number(index)];
}

function noteName(note: number): string {
  const names = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
  return `${names[note % 12]}${Math.floor(note / 12) - 1}`;
}
