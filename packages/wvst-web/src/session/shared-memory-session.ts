import type {
  InstanceApi,
  InstanceDescriptor,
  StreamSharedMemoryCreateResult,
  StreamSharedMemoryDestroyResult,
  StreamSharedMemoryProcessOptions,
  StreamSharedMemoryProcessResult,
  StreamSharedMemoryPumpClearEventsResult,
  StreamSharedMemoryPumpEnqueueEventsOptions,
  StreamSharedMemoryPumpEnqueueEventsResult,
  StreamSharedMemoryPumpStartOptions,
  StreamSharedMemoryPumpStartResult,
  StreamSharedMemoryPumpStatusResult,
  StreamSharedMemoryPumpStopResult,
  StreamSharedMemoryStatusResult,
} from "../control/instances.js";

type WithoutInstanceId<T extends { instanceId: number }> = Omit<T, "instanceId">;

export interface WVSTSharedMemoryPumpSessionOptions {
  instances: InstanceApi;
  instance: InstanceDescriptor;
  capacityBlocks?: number;
  startPump?: boolean;
  pump?: WithoutInstanceId<StreamSharedMemoryPumpStartOptions>;
}

export interface WVSTSharedMemoryPumpSession {
  instance: InstanceDescriptor;
  descriptor: StreamSharedMemoryCreateResult;
  startPump(
    options?: WithoutInstanceId<StreamSharedMemoryPumpStartOptions>,
  ): Promise<StreamSharedMemoryPumpStartResult>;
  stopPump(): Promise<StreamSharedMemoryPumpStopResult>;
  sharedMemoryStatus(): Promise<StreamSharedMemoryStatusResult>;
  pumpStatus(): Promise<StreamSharedMemoryPumpStatusResult>;
  process(
    options?: WithoutInstanceId<StreamSharedMemoryProcessOptions>,
  ): Promise<StreamSharedMemoryProcessResult>;
  enqueueEvents(
    options: WithoutInstanceId<StreamSharedMemoryPumpEnqueueEventsOptions>,
  ): Promise<StreamSharedMemoryPumpEnqueueEventsResult>;
  clearEvents(): Promise<StreamSharedMemoryPumpClearEventsResult>;
  destroy(): Promise<StreamSharedMemoryDestroyResult | null>;
}

export async function createWVSTSharedMemoryPumpSession(
  options: WVSTSharedMemoryPumpSessionOptions,
): Promise<WVSTSharedMemoryPumpSession> {
  const { instances, instance } = options;
  const descriptor = await instances.sharedMemoryCreate({
    instanceId: instance.instanceId,
    capacityBlocks: options.capacityBlocks,
  });
  let pumpRunning = false;

  try {
    if (options.startPump ?? true) {
      const start = await instances.sharedMemoryPumpStart({
        instanceId: instance.instanceId,
        ...options.pump,
      });
      pumpRunning = start.started;
    }
  } catch (error) {
    await instances.sharedMemoryDestroy({ instanceId: instance.instanceId });
    throw error;
  }

  let destroyed = false;
  const assertActive = () => {
    if (destroyed) {
      throw new Error("WVST shared memory pump session is destroyed");
    }
  };

  return {
    instance,
    descriptor,
    startPump: async (startOptions = {}) => {
      assertActive();
      const result = await instances.sharedMemoryPumpStart({
        instanceId: instance.instanceId,
        ...startOptions,
      });
      pumpRunning = result.started;
      return result;
    },
    stopPump: async () => {
      assertActive();
      const result = await instances.sharedMemoryPumpStop({
        instanceId: instance.instanceId,
      });
      pumpRunning = false;
      return result;
    },
    sharedMemoryStatus: () => {
      assertActive();
      return instances.sharedMemoryStatus({ instanceId: instance.instanceId });
    },
    pumpStatus: () => {
      assertActive();
      return instances.sharedMemoryPumpStatus({ instanceId: instance.instanceId });
    },
    process: (processOptions = {}) => {
      assertActive();
      return instances.sharedMemoryProcess({
        instanceId: instance.instanceId,
        ...processOptions,
      });
    },
    enqueueEvents: (eventOptions) => {
      assertActive();
      return instances.sharedMemoryPumpEnqueueEvents({
        instanceId: instance.instanceId,
        ...eventOptions,
      });
    },
    clearEvents: () => {
      assertActive();
      return instances.sharedMemoryPumpClearEvents({
        instanceId: instance.instanceId,
      });
    },
    destroy: async () => {
      if (destroyed) {
        return null;
      }
      destroyed = true;
      try {
        if (pumpRunning) {
          await instances.sharedMemoryPumpStop({ instanceId: instance.instanceId });
        }
      } finally {
        pumpRunning = false;
      }
      return instances.sharedMemoryDestroy({ instanceId: instance.instanceId });
    },
  };
}
