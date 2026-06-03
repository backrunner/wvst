import {
  createWVSTAudioDeviceSession,
  type InstanceDescriptor,
  type PluginDescriptor,
  type Vst3ParameterInfo,
  type WVSTAudioDeviceSession,
} from "@wvst/web";
import {
  element,
  fillSelect,
  optionValue,
  readInteger,
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

let clients: BridgeClients | undefined;
let plugins: PluginDescriptor[] = [];
let classes: PluginClassChoice[] = [];
let context: AudioContext | undefined;
let instance: InstanceDescriptor | undefined;
let session: WVSTAudioDeviceSession | undefined;
let metricsTimer: number | undefined;

element("connect").addEventListener("click", () => run(connect));
element("rescan").addEventListener("click", () => run(() => scan(true)));
element("plugin").addEventListener("change", () => run(refreshClasses));
element("mount").addEventListener("click", () => run(mount));
element("stop").addEventListener("click", () => run(stop));
window.addEventListener("pagehide", () => void stop());

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
    inputChannels: readInteger("inputs", 2),
    outputChannels: readInteger("outputs", 2),
  });
  await clients.client.instances.start({ instanceId: created.instanceId });
  instance = await clients.client.instances.openStream({ instanceId: created.instanceId });
  session = await createWVSTAudioDeviceSession({
    context,
    bridgeWorker: clients.worker,
    instance,
    processorUrl: processorUrl(),
    frames: instance.maxBlockFrames,
    inputDeviceId: optionValue("input-device"),
    outputDeviceId: optionValue("output-device"),
    startOutput: true,
    autoRestartAudioStream: true,
  });
  await renderParameters();
  showJson("runtime", await clients.client.instances.runtimeSnapshot({ instanceId: instance.instanceId }));
  startMetrics();
  setButton("stop", true);
  setStatus("Mounted", "ok");
}

async function renderParameters(): Promise<void> {
  if (!clients || !instance) {
    return;
  }
  const result = await clients.client.instances.parameters({ instanceId: instance.instanceId });
  const container = element<HTMLDivElement>("parameters");
  container.innerHTML = "";
  for (const parameter of result.parameters.slice(0, 64)) {
    container.append(await parameterControl(parameter));
  }
}

async function parameterControl(parameter: Vst3ParameterInfo): Promise<HTMLElement> {
  const row = document.createElement("div");
  row.className = "parameter";
  const label = document.createElement("span");
  label.textContent = parameter.title ?? `Parameter ${parameter.id}`;
  const slider = document.createElement("input");
  slider.type = "range";
  slider.min = "0";
  slider.max = "1";
  slider.step = parameter.stepCount > 0 ? String(1 / parameter.stepCount) : "0.001";
  slider.disabled = parameter.flags.readOnly;
  const value = document.createElement("strong");
  await updateParameterValue(parameter, slider, value);
  bindParameterEdit(parameter, slider, value);
  row.append(label, slider, value);
  return row;
}

function bindParameterEdit(
  parameter: Vst3ParameterInfo,
  slider: HTMLInputElement,
  value: HTMLElement,
): void {
  let editing = false;
  slider.addEventListener("pointerdown", () => {
    editing = true;
    void clients?.client.instances.parameterBeginEdit({
      instanceId: requireInstance().instanceId,
      parameterId: parameter.id,
    });
  });
  slider.addEventListener("input", () => {
    const valueNormalized = Number(slider.value);
    value.textContent = valueNormalized.toFixed(3);
    const request = editing
      ? clients?.client.instances.parameterPerformEdit({
          instanceId: requireInstance().instanceId,
          parameterId: parameter.id,
          valueNormalized,
        })
      : clients?.client.instances.parameterSet({
          instanceId: requireInstance().instanceId,
          parameterId: parameter.id,
          valueNormalized,
        });
    void request?.then(() => updateParameterValue(parameter, slider, value));
  });
  slider.addEventListener("pointerup", () => {
    editing = false;
    void clients?.client.instances.parameterEndEdit({
      instanceId: requireInstance().instanceId,
      parameterId: parameter.id,
    });
  });
}

async function updateParameterValue(
  parameter: Vst3ParameterInfo,
  slider: HTMLInputElement,
  value: HTMLElement,
): Promise<void> {
  if (!clients || !instance) {
    return;
  }
  const info = await clients.client.instances.parameterInfo({
    instanceId: instance.instanceId,
    parameterId: parameter.id,
  });
  slider.value = String(info.valueNormalized);
  value.textContent = info.valueString ?? info.valueNormalized.toFixed(3);
}

async function populateDevices(): Promise<void> {
  const devices = await listDevices();
  fillSelect(
    "input-device",
    devices.inputs.map((device, index) => ({
      value: device.deviceId,
      label: deviceLabel(device, `Input ${index + 1}`),
    })),
    "Default input",
  );
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
  await session?.stop();
  session = undefined;
  await destroyInstance(clients?.client, instance);
  instance = undefined;
  context?.close();
  context = undefined;
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

function requireInstance(): InstanceDescriptor {
  if (!instance) {
    throw new Error("WVST instance is not mounted");
  }
  return instance;
}
