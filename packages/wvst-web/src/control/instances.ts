import type * as Control from "./instance-control.js";
import type * as Shared from "./shared-memory.js";

export * from "./instance-control.js";
export * from "./shared-memory.js";

export interface InstanceApi {
  create(options: Control.InstanceCreateOptions): Promise<Control.InstanceDescriptor>;
  list(): Promise<Control.InstanceDescriptor[]>;
  status(options: Control.InstanceStatusOptions): Promise<Control.InstanceStatusResult>;
  metadataRefresh(
    options: Control.InstanceMetadataRefreshOptions,
  ): Promise<Control.InstanceMetadataRefreshResult>;
  runtimeSnapshot(
    options: Control.InstanceRuntimeSnapshotOptions,
  ): Promise<Control.InstanceRuntimeSnapshotResult>;
  restart(options: Control.InstanceRestartOptions): Promise<Control.InstanceRestartResult>;
  start(options: Control.InstanceProcessingOptions): Promise<Control.InstanceProcessingResult>;
  stop(options: Control.InstanceProcessingOptions): Promise<Control.InstanceProcessingResult>;
  parameters(options: Control.InstanceParametersOptions): Promise<Control.InstanceParametersResult>;
  parameterGet(options: Control.InstanceParameterGetOptions): Promise<Control.InstanceParameterGetResult>;
  parameterInfo(options: Control.InstanceParameterInfoOptions): Promise<Control.InstanceParameterInfoResult>;
  parameterValueByString(
    options: Control.InstanceParameterValueByStringOptions,
  ): Promise<Control.InstanceParameterValueByStringResult>;
  parameterNormalizedByPlain(
    options: Control.InstanceParameterNormalizedByPlainOptions,
  ): Promise<Control.InstanceParameterNormalizedByPlainResult>;
  parameterSet(options: Control.InstanceParameterSetOptions): Promise<Control.InstanceParameterSetResult>;
  parameterBeginEdit(
    options: Control.InstanceParameterBeginEditOptions,
  ): Promise<Control.InstanceParameterBeginEditResult>;
  parameterPerformEdit(
    options: Control.InstanceParameterPerformEditOptions,
  ): Promise<Control.InstanceParameterPerformEditResult>;
  parameterEndEdit(
    options: Control.InstanceParameterEndEditOptions,
  ): Promise<Control.InstanceParameterEndEditResult>;
  parameterEdit(
    options: Control.InstanceParameterEditOptions,
  ): Promise<Control.InstanceParameterEditAggregateResult>;
  units(options: Control.InstanceUnitsOptions): Promise<Control.InstanceUnitsResult>;
  selectUnit(options: Control.InstanceSelectUnitOptions): Promise<Control.InstanceSelectUnitResult>;
  unitByBus(options: Control.InstanceUnitByBusOptions): Promise<Control.InstanceUnitByBusResult>;
  setUnitProgramData(
    options: Control.InstanceSetUnitProgramDataOptions,
  ): Promise<Control.InstanceSetUnitProgramDataResult>;
  setUnitProgramDataAndRefresh(
    options: Control.InstanceSetUnitProgramDataAndRefreshOptions,
  ): Promise<Control.InstanceSetUnitProgramDataAndRefreshResult>;
  programDataSupported(
    options: Control.InstanceProgramDataOptions,
  ): Promise<Control.InstanceProgramDataSupportedResult>;
  getProgramData(options: Control.InstanceProgramDataOptions): Promise<Control.InstanceProgramDataResult>;
  setProgramData(options: Control.InstanceSetProgramDataOptions): Promise<Control.InstanceSetProgramDataResult>;
  setProgramDataAndRefresh(
    options: Control.InstanceSetProgramDataAndRefreshOptions,
  ): Promise<Control.InstanceSetProgramDataAndRefreshResult>;
  unitDataSupported(options: Control.InstanceUnitDataOptions): Promise<Control.InstanceUnitDataSupportedResult>;
  getUnitData(options: Control.InstanceUnitDataOptions): Promise<Control.InstanceUnitDataResult>;
  setUnitData(options: Control.InstanceSetUnitDataOptions): Promise<Control.InstanceSetUnitDataResult>;
  setUnitDataAndRefresh(
    options: Control.InstanceSetUnitDataAndRefreshOptions,
  ): Promise<Control.InstanceSetUnitDataAndRefreshResult>;
  getState(options: Control.InstanceGetStateOptions): Promise<Control.InstanceGetStateResult>;
  setState(options: Control.InstanceSetStateOptions): Promise<Control.InstanceSetStateResult>;
  setStateAndRefresh(
    options: Control.InstanceSetStateAndRefreshOptions,
  ): Promise<Control.InstanceSetStateAndRefreshResult>;
  notifyComponent(
    options: Control.InstanceConnectionNotifyOptions,
  ): Promise<Control.InstanceConnectionNotifyResult>;
  notifyController(
    options: Control.InstanceConnectionNotifyOptions,
  ): Promise<Control.InstanceConnectionNotifyResult>;
  notifyComponentAndRefresh(
    options: Control.InstanceConnectionNotifyAndRefreshOptions,
  ): Promise<Control.InstanceConnectionNotifyAndRefreshResult>;
  notifyControllerAndRefresh(
    options: Control.InstanceConnectionNotifyAndRefreshOptions,
  ): Promise<Control.InstanceConnectionNotifyAndRefreshResult>;
  destroy(options: Control.InstanceDestroyOptions): Promise<Control.InstanceDestroyResult>;
  openStream(options: Control.StreamLifecycleOptions): Promise<Control.InstanceDescriptor>;
  closeStream(options: Control.StreamLifecycleOptions): Promise<Control.InstanceDescriptor>;
  sharedMemoryCreate(
    options: Shared.StreamSharedMemoryCreateOptions,
  ): Promise<Shared.StreamSharedMemoryCreateResult>;
  sharedMemoryDestroy(
    options: Control.StreamLifecycleOptions,
  ): Promise<Shared.StreamSharedMemoryDestroyResult>;
  sharedMemoryStatus(
    options: Control.StreamLifecycleOptions,
  ): Promise<Shared.StreamSharedMemoryStatusResult>;
  sharedMemoryProcess(
    options: Shared.StreamSharedMemoryProcessOptions,
  ): Promise<Shared.StreamSharedMemoryProcessResult>;
  sharedMemoryPumpStart(
    options: Shared.StreamSharedMemoryPumpStartOptions,
  ): Promise<Shared.StreamSharedMemoryPumpStartResult>;
  sharedMemoryPumpStop(
    options: Control.StreamLifecycleOptions,
  ): Promise<Shared.StreamSharedMemoryPumpStopResult>;
  sharedMemoryPumpStatus(
    options: Control.StreamLifecycleOptions,
  ): Promise<Shared.StreamSharedMemoryPumpStatusResult>;
  sharedMemoryPumpEnqueueEvents(
    options: Shared.StreamSharedMemoryPumpEnqueueEventsOptions,
  ): Promise<Shared.StreamSharedMemoryPumpEnqueueEventsResult>;
  sharedMemoryPumpClearEvents(
    options: Control.StreamLifecycleOptions,
  ): Promise<Shared.StreamSharedMemoryPumpClearEventsResult>;
}
