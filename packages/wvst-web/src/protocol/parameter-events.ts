export const PARAMETER_AUTOMATION_EVENT_BYTES = 16;

export interface ParameterAutomationEvent {
  sampleOffset: number;
  parameterId: number;
  valueNormalized: number;
}

export function parameterAutomationEventPayloadBytes(eventCount: number): number {
  return checkedU32(
    eventCount * PARAMETER_AUTOMATION_EVENT_BYTES,
    "parameter automation event payload bytes",
  );
}

export function encodeParameterAutomationEvent(
  event: ParameterAutomationEvent,
): ArrayBuffer {
  validateParameterAutomationEvent(event);
  const buffer = new ArrayBuffer(PARAMETER_AUTOMATION_EVENT_BYTES);
  const view = new DataView(buffer);

  view.setUint16(0, event.sampleOffset, true);
  view.setUint32(4, event.parameterId, true);
  view.setFloat64(8, event.valueNormalized, true);

  return buffer;
}

export function decodeParameterAutomationEvent(
  buffer: ArrayBufferLike,
): ParameterAutomationEvent {
  if (buffer.byteLength < PARAMETER_AUTOMATION_EVENT_BYTES) {
    throw new Error(
      `WVST parameter automation event requires ${PARAMETER_AUTOMATION_EVENT_BYTES} bytes`,
    );
  }

  const view = new DataView(buffer, 0, PARAMETER_AUTOMATION_EVENT_BYTES);
  const event: ParameterAutomationEvent = {
    sampleOffset: view.getUint16(0, true),
    parameterId: view.getUint32(4, true),
    valueNormalized: view.getFloat64(8, true),
  };
  validateParameterAutomationEvent(event);
  return event;
}

export function encodeParameterAutomationEvents(
  events: ParameterAutomationEvent[],
): ArrayBuffer {
  const payload = new Uint8Array(parameterAutomationEventPayloadBytes(events.length));

  for (let index = 0; index < events.length; index += 1) {
    payload.set(
      new Uint8Array(encodeParameterAutomationEvent(events[index])),
      index * PARAMETER_AUTOMATION_EVENT_BYTES,
    );
  }

  return payload.buffer;
}

export function decodeParameterAutomationEvents(
  payload: ArrayBufferLike,
  eventCount: number,
): ParameterAutomationEvent[] {
  const expectedBytes = parameterAutomationEventPayloadBytes(eventCount);
  if (payload.byteLength !== expectedBytes) {
    throw new Error(
      `WVST parameter automation event payload length mismatch: expected ${expectedBytes}, got ${payload.byteLength}`,
    );
  }

  const events: ParameterAutomationEvent[] = [];
  for (
    let offset = 0;
    offset < expectedBytes;
    offset += PARAMETER_AUTOMATION_EVENT_BYTES
  ) {
    events.push(
      decodeParameterAutomationEvent(
        payload.slice(offset, offset + PARAMETER_AUTOMATION_EVENT_BYTES),
      ),
    );
  }

  return events;
}

function validateParameterAutomationEvent(event: ParameterAutomationEvent): void {
  if (!Number.isInteger(event.sampleOffset) || event.sampleOffset < 0 || event.sampleOffset > 0xffff) {
    throw new Error(`invalid WVST parameter sample offset: ${event.sampleOffset}`);
  }
  if (!Number.isInteger(event.parameterId) || event.parameterId < 0 || event.parameterId > 0xffffffff) {
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

function checkedU32(value: number, label: string): number {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
    throw new Error(`invalid WVST ${label}: ${value}`);
  }

  return value;
}
