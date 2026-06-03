import type {
  InstanceDescriptor,
  InstanceGetStateResult,
  InstanceSetStateOptions,
} from "./instances.js";

export const WVST_INSTANCE_STATE_SNAPSHOT_SCHEMA_VERSION = 1;

export interface WVSTInstanceStateSnapshot {
  schemaVersion: typeof WVST_INSTANCE_STATE_SNAPSHOT_SCHEMA_VERSION;
  pluginId?: string;
  classId?: string;
  sampleRate?: number;
  maxBlockFrames?: number;
  inputChannels?: number;
  outputChannels?: number;
  inputBusIndex?: number;
  outputBusIndex?: number;
  componentStateBase64: string | null;
  controllerStateBase64: string | null;
}

export interface WVSTInstanceStateSnapshotCompatibility {
  compatible: boolean;
  reasons: string[];
}

export function createWVSTInstanceStateSnapshot(
  state: InstanceGetStateResult,
  descriptor?: InstanceDescriptor,
): WVSTInstanceStateSnapshot {
  return {
    schemaVersion: WVST_INSTANCE_STATE_SNAPSHOT_SCHEMA_VERSION,
    pluginId: descriptor?.pluginId,
    classId: descriptor?.classId,
    sampleRate: descriptor?.sampleRate,
    maxBlockFrames: descriptor?.maxBlockFrames,
    inputChannels: descriptor?.inputChannels,
    outputChannels: descriptor?.outputChannels,
    inputBusIndex: descriptor?.inputBusIndex,
    outputBusIndex: descriptor?.outputBusIndex,
    componentStateBase64: state.componentStateBase64,
    controllerStateBase64: state.controllerStateBase64,
  };
}

export function instanceStateSnapshotToSetStateOptions(
  instanceId: number,
  snapshot: WVSTInstanceStateSnapshot,
  descriptor?: InstanceDescriptor,
): InstanceSetStateOptions {
  if (snapshot.schemaVersion !== WVST_INSTANCE_STATE_SNAPSHOT_SCHEMA_VERSION) {
    throw new Error(
      `unsupported WVST instance state snapshot schema version: ${snapshot.schemaVersion}`,
    );
  }
  if (descriptor !== undefined) {
    const compatibility = checkWVSTInstanceStateSnapshotCompatibility(snapshot, descriptor);
    if (!compatibility.compatible) {
      throw new Error(
        `WVST instance state snapshot is not compatible with the target instance: ${compatibility.reasons.join("; ")}`,
      );
    }
  }

  const options: InstanceSetStateOptions = { instanceId };
  if (snapshot.componentStateBase64 !== null) {
    options.componentStateBase64 = snapshot.componentStateBase64;
  }
  if (snapshot.controllerStateBase64 !== null) {
    options.controllerStateBase64 = snapshot.controllerStateBase64;
  }
  if (
    options.componentStateBase64 === undefined &&
    options.controllerStateBase64 === undefined
  ) {
    throw new Error("WVST instance state snapshot does not contain restorable state");
  }

  return options;
}

export function checkWVSTInstanceStateSnapshotCompatibility(
  snapshot: WVSTInstanceStateSnapshot,
  descriptor: InstanceDescriptor,
): WVSTInstanceStateSnapshotCompatibility {
  const reasons: string[] = [];
  checkStringField(reasons, "pluginId", snapshot.pluginId, descriptor.pluginId);
  checkStringField(reasons, "classId", snapshot.classId, descriptor.classId);
  checkNumberField(reasons, "sampleRate", snapshot.sampleRate, descriptor.sampleRate);
  checkNumberField(
    reasons,
    "maxBlockFrames",
    snapshot.maxBlockFrames,
    descriptor.maxBlockFrames,
  );
  checkNumberField(reasons, "inputChannels", snapshot.inputChannels, descriptor.inputChannels);
  checkNumberField(reasons, "outputChannels", snapshot.outputChannels, descriptor.outputChannels);
  checkNumberField(reasons, "inputBusIndex", snapshot.inputBusIndex, descriptor.inputBusIndex);
  checkNumberField(
    reasons,
    "outputBusIndex",
    snapshot.outputBusIndex,
    descriptor.outputBusIndex,
  );

  return {
    compatible: reasons.length === 0,
    reasons,
  };
}

function checkStringField(
  reasons: string[],
  field: string,
  expected: string | undefined,
  actual: string | undefined,
): void {
  if (expected !== undefined && actual !== undefined && expected !== actual) {
    reasons.push(`${field} mismatch: expected ${expected}, got ${actual}`);
  }
}

function checkNumberField(
  reasons: string[],
  field: string,
  expected: number | undefined,
  actual: number | undefined,
): void {
  if (expected !== undefined && actual === undefined) {
    reasons.push(`${field} mismatch: expected ${expected}, got unset`);
    return;
  }
  if (expected !== undefined && expected !== actual) {
    reasons.push(`${field} mismatch: expected ${expected}, got ${actual}`);
  }
}
