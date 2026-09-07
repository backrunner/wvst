import {
  WVSTBridgeWorkerClient,
  WVSTClient,
  listWVSTAudioDevices,
  type InstanceDescriptor,
  type PluginDescriptor,
  type PluginFactoryClass,
  type WVSTAudioDeviceList,
} from "@wvst/web";

export interface BridgeClients {
  client: WVSTClient;
  worker: WVSTBridgeWorkerClient;
  transportWorker: Worker;
}

export interface PluginClassChoice {
  classId: string;
  label: string;
}

export async function connectBridge(
  endpoint: string,
  token: string | undefined,
): Promise<BridgeClients> {
  const client = await WVSTClient.connect({
    endpoint,
    token,
    requireLowLatency: true,
  });
  let transportWorker: Worker | undefined;
  try {
    transportWorker = new Worker(sdkAssetUrl("bridge-worker.js"), { type: "module" });
    const worker = new WVSTBridgeWorkerClient({ worker: transportWorker });
    await worker.connect(endpoint);
    await worker.request("bridge.hello", {
      ...client.createHelloRequest().params,
      token,
    });
    return { client, worker, transportWorker };
  } catch (error) {
    transportWorker?.terminate();
    client.close();
    throw error;
  }
}

export async function stopBridge(clients: BridgeClients | undefined): Promise<void> {
  if (!clients) {
    return;
  }
  try {
    await clients.worker.close();
  } finally {
    clients.transportWorker.terminate();
    clients.client.close();
  }
}

export async function listDevices(): Promise<WVSTAudioDeviceList> {
  try {
    return await listWVSTAudioDevices();
  } catch {
    return { inputs: [], outputs: [] };
  }
}

export async function classChoices(
  client: WVSTClient,
  plugin: PluginDescriptor,
): Promise<PluginClassChoice[]> {
  const factory = await client.plugins.factoryInfo({ path: plugin.path });
  return factory.classes.map(factoryClassChoice).filter(hasClassId);
}

export function pluginLabel(plugin: PluginDescriptor): string {
  return [plugin.vendor, plugin.name].filter(Boolean).join(" / ");
}

export function deviceLabel(device: MediaDeviceInfo, fallback: string): string {
  return device.label || fallback;
}

export function processorUrl(): string {
  return sdkAssetUrl("loopback-processor.js");
}

export async function destroyInstance(
  client: WVSTClient | undefined,
  instance: InstanceDescriptor | undefined,
): Promise<void> {
  if (!client || !instance) {
    return;
  }
  await client.instances.destroy({ instanceId: instance.instanceId });
}

function sdkAssetUrl(file: string): string {
  return new URL(`/packages/wvst-web/dist/esm/audio/${file}`, location.origin).href;
}

function factoryClassChoice(factoryClass: PluginFactoryClass): PluginClassChoice {
  const category = factoryClass.category ? ` (${factoryClass.category})` : "";
  return {
    classId: factoryClass.classId,
    label: `${factoryClass.name ?? factoryClass.classId}${category}`,
  };
}

function hasClassId(choice: PluginClassChoice): boolean {
  return choice.classId.length > 0;
}
