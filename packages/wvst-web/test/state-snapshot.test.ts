import { describe, expect, it } from "vitest";
import {
  checkWVSTInstanceStateSnapshotCompatibility,
  createWVSTInstanceStateSnapshot,
  instanceStateSnapshotToSetStateOptions,
  type InstanceDescriptor,
} from "../src/index.js";

const descriptor: InstanceDescriptor = {
  instanceId: 1,
  streamId: 2,
  pluginId: "plugin-a",
  pluginPath: "/Library/Audio/Plug-Ins/VST3/PluginA.vst3",
  classId: "class-a",
  sampleRate: 48_000,
  maxBlockFrames: 128,
  inputChannels: 2,
  outputChannels: 2,
  state: "ready",
  workerState: "ready",
  streamState: "open",
  runtimeCapabilities: {
    schemaVersion: 2,
    binaryAudioProcess: true,
    componentState: true,
    controller: true,
    controllerState: true,
    parameters: true,
    parameterAutomation: true,
    units: false,
    unitProgramData: false,
    programListData: false,
    unitData: false,
    midiMapping: true,
    outputEvents: true,
    outputParameterChanges: true,
    componentHandlerEvents: true,
    connectionPoints: true,
    processContext: true,
  },
  latencySamples: 0,
  tailSamples: 0,
  tailInfo: { kind: "none", samples: 0 },
};

describe("WVST state snapshots", () => {
  it("creates restorable state options from compatible snapshots", () => {
    const snapshot = createWVSTInstanceStateSnapshot(
      {
        instanceId: descriptor.instanceId,
        componentStateBase64: "Y29tcA==",
        controllerStateBase64: "Y3RybA==",
      },
      descriptor,
    );

    expect(instanceStateSnapshotToSetStateOptions(9, snapshot, descriptor)).toEqual({
      instanceId: 9,
      componentStateBase64: "Y29tcA==",
      controllerStateBase64: "Y3RybA==",
    });
  });

  it("reports descriptor mismatches before restoring plugin state", () => {
    const snapshot = createWVSTInstanceStateSnapshot(
      {
        instanceId: descriptor.instanceId,
        componentStateBase64: "Y29tcA==",
        controllerStateBase64: null,
      },
      descriptor,
    );
    const incompatible = {
      ...descriptor,
      pluginId: "plugin-b",
      sampleRate: 44_100,
    };

    const result = checkWVSTInstanceStateSnapshotCompatibility(snapshot, incompatible);

    expect(result.compatible).toBe(false);
    expect(result.reasons).toEqual([
      "pluginId mismatch: expected plugin-a, got plugin-b",
      "sampleRate mismatch: expected 48000, got 44100",
    ]);
    expect(() => instanceStateSnapshotToSetStateOptions(9, snapshot, incompatible)).toThrow(
      /not compatible/,
    );
  });

  it("rejects empty snapshots without component or controller state", () => {
    expect(() =>
      instanceStateSnapshotToSetStateOptions(1, {
        schemaVersion: 1,
        componentStateBase64: null,
        controllerStateBase64: null,
      }),
    ).toThrow(/does not contain restorable state/);
  });
});
