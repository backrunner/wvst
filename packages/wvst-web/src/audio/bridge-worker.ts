import { WebSocketRpcTransport, type JsonValue } from "../client/transport.js";
import { AudioStreamPump } from "./audio-stream-pump.js";
import type { MidiEvent, ParameterAutomationEvent } from "../protocol/index.js";
import type {
  BridgeWorkerCommand,
  DedicatedWorkerScope,
} from "./bridge-worker-messages.js";

const workerScope = globalThis as unknown as DedicatedWorkerScope;
let transport: WebSocketRpcTransport | undefined;
const audioPumps = new Map<number, AudioStreamPump>();

workerScope.addEventListener("message", (event) => {
  void handleCommand(event.data);
});

async function handleCommand(command: BridgeWorkerCommand): Promise<void> {
  try {
    switch (command.type) {
      case "connect":
        transport = await WebSocketRpcTransport.connect(command.endpoint, command.transportOptions);
        postResult(command.id, null);
        return;
      case "request":
        postResult(command.id, await requireTransport().request(command.method, command.params));
        return;
      case "sendBinary": {
        const echoed = await requireTransport().sendBinary(command.frame);
        postResult(command.id, echoed, [echoed]);
        return;
      }
      case "startAudioStream":
        startAudioStream(command);
        postResult(command.id, null);
        return;
      case "stopAudioStream":
        stopAudioStream(command.streamId);
        postResult(command.id, null);
        return;
      case "sendMidiEvents":
        enqueueMidiEvents(command.streamId, command.events);
        postResult(command.id, null);
        return;
      case "sendParameterEvents":
        enqueueParameterEvents(command.streamId, command.events);
        postResult(command.id, null);
        return;
      case "close":
        stopAllAudioStreams();
        requireTransport().close();
        transport = undefined;
        postResult(command.id, null);
        return;
    }
  } catch (error) {
    postError(command.id, error);
  }
}

function requireTransport(): WebSocketRpcTransport {
  if (!transport) {
    throw new Error("WVST bridge worker is not connected");
  }

  return transport;
}

function postResult(id: number, result: JsonValue | ArrayBuffer | null, transfer?: Transferable[]) {
  workerScope.postMessage({ id, ok: true, result }, transfer);
}

function postError(id: number, error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  workerScope.postMessage({ id, ok: false, error: message });
}

function startAudioStream(command: Extract<BridgeWorkerCommand, { type: "startAudioStream" }>) {
  stopAudioStream(command.streamId);
  const pump = new AudioStreamPump(command, (frame) => requireTransport().sendBinary(frame));
  audioPumps.set(command.streamId, pump);
  pump.start();
}

function stopAudioStream(streamId: number) {
  const pump = audioPumps.get(streamId);
  if (pump) {
    pump.stop();
    audioPumps.delete(streamId);
  }
}

function enqueueMidiEvents(streamId: number, events: MidiEvent[]) {
  const pump = audioPumps.get(streamId);
  if (!pump) {
    throw new Error(`WVST audio stream is not active: ${streamId}`);
  }

  pump.enqueueMidiEvents(events);
}

function enqueueParameterEvents(streamId: number, events: ParameterAutomationEvent[]) {
  const pump = audioPumps.get(streamId);
  if (!pump) {
    throw new Error(`WVST audio stream is not active: ${streamId}`);
  }

  pump.enqueueParameterEvents(events);
}

function stopAllAudioStreams() {
  for (const pump of audioPumps.values()) {
    pump.stop();
  }
  audioPumps.clear();
}
