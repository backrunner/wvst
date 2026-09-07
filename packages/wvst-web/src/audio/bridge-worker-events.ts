import type { MidiEvent, ParameterAutomationEvent } from "../protocol/index.js";
import { validateMidiEvent } from "../protocol/midi-events.js";
import { LoopbackCounter } from "./loopback.js";

export interface BridgeWorkerEventQueueOptions {
  frames: number;
  counters: Int32Array;
  nextInputTargetSequence(): number;
}

interface QueuedAudioEvent<TEvent> {
  event: TEvent;
  targetSequence: number;
}

const MAX_PENDING_MIDI_EVENTS = 4_096;
const MAX_PENDING_PARAMETER_EVENTS = 4_096;

export class BridgeWorkerEventQueue {
  private pendingMidiEvents: QueuedAudioEvent<MidiEvent>[] = [];
  private pendingParameterEvents: QueuedAudioEvent<ParameterAutomationEvent>[] = [];

  constructor(private readonly options: BridgeWorkerEventQueueOptions) {}

  enqueueMidiEvents(events: MidiEvent[]): void {
    if (this.pendingMidiEvents.length + events.length > MAX_PENDING_MIDI_EVENTS) {
      Atomics.add(this.options.counters, LoopbackCounter.Overflows, 1);
      Atomics.add(this.options.counters, LoopbackCounter.DroppedMidiEvents, events.length);
      throw new Error("WVST MIDI event queue is full");
    }

    for (const event of events) {
      this.validateSampleOffset("MIDI", event.sampleOffset);
      validateMidiEvent(event);
    }
    const targetSequence = this.options.nextInputTargetSequence();
    for (const event of events) {
      this.pendingMidiEvents.push({ event, targetSequence });
    }
  }

  enqueueParameterEvents(events: ParameterAutomationEvent[]): void {
    if (
      this.pendingParameterEvents.length + events.length >
      MAX_PENDING_PARAMETER_EVENTS
    ) {
      Atomics.add(this.options.counters, LoopbackCounter.Overflows, 1);
      Atomics.add(
        this.options.counters,
        LoopbackCounter.DroppedParameterEvents,
        events.length,
      );
      throw new Error("WVST parameter event queue is full");
    }

    for (const event of events) {
      this.validateSampleOffset("parameter", event.sampleOffset);
      validateParameterEvent(event);
    }
    const targetSequence = this.options.nextInputTargetSequence();
    for (const event of events) {
      this.pendingParameterEvents.push({ event, targetSequence });
    }
  }

  takeMidiEvents(readSequence: number): MidiEvent[] {
    if (this.pendingMidiEvents.length === 0) {
      return [];
    }

    // Late MIDI changes instrument state: losing note-off or pedal-up can hang voices.
    // Preserve block/event order and apply overdue events at the start of the next block.
    this.pendingMidiEvents.sort((left, right) =>
      left.targetSequence - right.targetSequence || left.event.sampleOffset - right.event.sampleOffset);
    const due: MidiEvent[] = [];
    const pending: QueuedAudioEvent<MidiEvent>[] = [];
    let late = 0;
    for (const item of this.pendingMidiEvents) {
      if (item.targetSequence > readSequence) pending.push(item);
      else if (item.targetSequence < readSequence) {
        late += 1;
        due.push({ ...item.event, sampleOffset: 0 });
      } else due.push(item.event);
    }
    this.pendingMidiEvents = pending;
    if (late > 0) Atomics.add(this.options.counters, LoopbackCounter.LateMidiEvents, late);
    return due;
  }

  takeParameterEvents(readSequence: number): ParameterAutomationEvent[] {
    if (this.pendingParameterEvents.length === 0) {
      return [];
    }

    const { due, pending, late } = takeDueEvents(
      this.pendingParameterEvents,
      readSequence,
    );
    this.pendingParameterEvents = pending;
    if (late > 0) {
      Atomics.add(this.options.counters, LoopbackCounter.LateParameterEvents, late);
      Atomics.add(this.options.counters, LoopbackCounter.DroppedParameterEvents, late);
    }
    due.sort((left, right) => {
      const byOffset = left.sampleOffset - right.sampleOffset;
      return byOffset === 0 ? left.parameterId - right.parameterId : byOffset;
    });
    return due;
  }

  private validateSampleOffset(label: string, sampleOffset: number): void {
    if (
      !Number.isInteger(sampleOffset) ||
      sampleOffset < 0 ||
      sampleOffset >= this.options.frames
    ) {
      throw new Error(
        `WVST ${label} sampleOffset must be an integer in [0, ${this.options.frames})`,
      );
    }
  }
}

function validateParameterEvent(event: ParameterAutomationEvent): void {
  if (
    !Number.isInteger(event.parameterId) ||
    event.parameterId < 0 ||
    event.parameterId > 0xffffffff
  ) {
    throw new Error(`invalid WVST parameter id: ${event.parameterId}`);
  }
  if (
    !Number.isFinite(event.valueNormalized) ||
    event.valueNormalized < 0 ||
    event.valueNormalized > 1
  ) {
    throw new Error(`invalid WVST normalized parameter value: ${event.valueNormalized}`);
  }
}

function takeDueEvents<TEvent>(
  events: QueuedAudioEvent<TEvent>[],
  readSequence: number,
): { due: TEvent[]; pending: QueuedAudioEvent<TEvent>[]; late: number } {
  const due: TEvent[] = [];
  const pending: QueuedAudioEvent<TEvent>[] = [];
  let late = 0;

  for (const item of events) {
    if (item.targetSequence < readSequence) {
      late += 1;
    } else if (item.targetSequence === readSequence) {
      due.push(item.event);
    } else {
      pending.push(item);
    }
  }

  return { due, pending, late };
}
