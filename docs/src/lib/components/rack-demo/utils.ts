import { WVSTClient, type PluginDescriptor } from '@wvst/web';
import type { PluginChoice, Prerequisites, RackParameter, RackSlot } from './types';

export function createPluginChoices(items: PluginDescriptor[]): PluginChoice[] {
  return items.flatMap((plugin) => {
    const classes = plugin.classes.length > 0
      ? plugin.classes
      : [{ name: plugin.name, subcategories: [], category: undefined, classId: undefined }];
    return classes.map((pluginClass, index) => {
      const category = (pluginClass.category ?? pluginClass.subcategories.join(' / ')) || 'VST3';
      const label = pluginClass.name && pluginClass.name !== plugin.name
        ? `${plugin.name} - ${pluginClass.name}`
        : plugin.name;
      return {
        key: `${plugin.pluginId}:${pluginClass.classId ?? index}`,
        pluginId: plugin.pluginId,
        ...(pluginClass.classId ? { classId: pluginClass.classId } : {}),
        label,
        sublabel: `${plugin.vendor ?? 'Unknown vendor'} / ${category}`,
        path: plugin.path,
        score: scorePluginChoice(`${label} ${category} ${pluginClass.subcategories.join(' ')}`)
      };
    });
  }).sort((left, right) => left.score - right.score || left.label.localeCompare(right.label));
}

export function readPrerequisites(): Prerequisites {
  const lowLatency = WVSTClient.lowLatencyPrerequisites();
  return {
    sharedArrayBuffer: lowLatency.sharedArrayBuffer,
    crossOriginIsolated: lowLatency.crossOriginIsolated,
    secureContext: window.isSecureContext === true
  };
}

export function allPrerequisitesMet(value: Prerequisites): boolean {
  return value.sharedArrayBuffer && value.crossOriginIsolated && value.secureContext;
}

export function updateRackParameter(
  items: RackSlot[],
  slotId: string,
  parameterId: number,
  update: (parameter: RackParameter) => RackParameter
): RackSlot[] {
  return items.map((slot) => slot.id !== slotId ? slot : {
    ...slot,
    parameters: slot.parameters.map((parameter) => parameter.info.id === parameterId ? update(parameter) : parameter)
  });
}

export function formatParameterValue(value: number, units: string | null): string {
  return `${Math.round(value * 100)}${units ? ` ${units}` : '%'}`;
}

export function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function safeDisconnect(node: AudioNode | undefined) {
  try {
    node?.disconnect();
  } catch {
    // The graph may already be torn down.
  }
}

export function createId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
}

function scorePluginChoice(value: string): number {
  const normalized = value.toLowerCase();
  if (/(effect|fx|audio module|processor)/.test(normalized)) return 0;
  if (/(instrument|synth|generator)/.test(normalized)) return 2;
  return 1;
}
