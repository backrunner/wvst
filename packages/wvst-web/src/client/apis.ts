import type {
  InstanceApi,
  InstanceConnectionNotifyAndRefreshOptions,
  InstanceConnectionNotifyAndRefreshResult,
  InstanceConnectionNotifyOptions,
  InstanceConnectionNotifyResult,
  InstanceCreateOptions,
  InstanceDescriptor,
  InstanceDestroyOptions,
  InstanceDestroyResult,
  InstanceGetStateOptions,
  InstanceGetStateResult,
  InstanceMetadataRefreshOptions,
  InstanceMetadataRefreshResult,
  InstanceParameterBeginEditOptions,
  InstanceParameterBeginEditResult,
  InstanceParameterEditAggregateResult,
  InstanceParameterEditOptions,
  InstanceParameterEndEditOptions,
  InstanceParameterEndEditResult,
  InstanceParameterGetOptions,
  InstanceParameterGetResult,
  InstanceParameterInfoOptions,
  InstanceParameterInfoResult,
  InstanceParameterNormalizedByPlainOptions,
  InstanceParameterNormalizedByPlainResult,
  InstanceParameterPerformEditOptions,
  InstanceParameterPerformEditResult,
  InstanceParameterSetOptions,
  InstanceParameterSetResult,
  InstanceParameterValueByStringOptions,
  InstanceParameterValueByStringResult,
  InstanceParametersOptions,
  InstanceParametersResult,
  InstanceProcessingOptions,
  InstanceProcessingResult,
  InstanceProgramDataOptions,
  InstanceProgramDataResult,
  InstanceProgramDataSupportedResult,
  InstanceRestartOptions,
  InstanceRestartResult,
  InstanceRuntimeSnapshotOptions,
  InstanceRuntimeSnapshotResult,
  InstanceSelectUnitOptions,
  InstanceSelectUnitResult,
  InstanceSetProgramDataAndRefreshOptions,
  InstanceSetProgramDataAndRefreshResult,
  InstanceSetProgramDataOptions,
  InstanceSetProgramDataResult,
  InstanceSetStateAndRefreshOptions,
  InstanceSetStateAndRefreshResult,
  InstanceSetStateOptions,
  InstanceSetStateResult,
  InstanceSetUnitDataAndRefreshOptions,
  InstanceSetUnitDataAndRefreshResult,
  InstanceSetUnitDataOptions,
  InstanceSetUnitDataResult,
  InstanceSetUnitProgramDataAndRefreshOptions,
  InstanceSetUnitProgramDataAndRefreshResult,
  InstanceSetUnitProgramDataOptions,
  InstanceSetUnitProgramDataResult,
  InstanceStatusOptions,
  InstanceStatusResult,
  InstanceUnitByBusOptions,
  InstanceUnitByBusResult,
  InstanceUnitDataOptions,
  InstanceUnitDataResult,
  InstanceUnitDataSupportedResult,
  InstanceUnitsOptions,
  InstanceUnitsResult,
  StreamLifecycleOptions,
  StreamSharedMemoryCreateOptions,
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
import type {
  PluginApi,
  PluginFactoryInfo,
  PluginFactoryInfoOptions,
  PluginListOptions,
  PluginScanOptions,
  PluginScanReport,
} from "../control/plugins.js";
import type { JsonValue } from "./transport.js";

export interface ApiRequester {
  request<T = JsonValue>(method: string, params: unknown): Promise<T>;
}

export function createInstanceApi(requester: ApiRequester): InstanceApi {
  return {
    create: (options: InstanceCreateOptions) =>
      requester.request<InstanceDescriptor>("instance.create", options),
    list: () => requester.request<InstanceDescriptor[]>("instance.list", {}),
    status: (options: InstanceStatusOptions) =>
      requester.request<InstanceStatusResult>("instance.status", options),
    metadataRefresh: (options: InstanceMetadataRefreshOptions) =>
      requester.request<InstanceMetadataRefreshResult>(
        "instance.metadata.refresh",
        options,
      ),
    runtimeSnapshot: (options: InstanceRuntimeSnapshotOptions) =>
      requester.request<InstanceRuntimeSnapshotResult>(
        "instance.runtime.snapshot",
        options,
      ),
    restart: (options: InstanceRestartOptions) =>
      requester.request<InstanceRestartResult>("instance.restart", options),
    start: (options: InstanceProcessingOptions) =>
      requester.request<InstanceProcessingResult>("instance.start", options),
    stop: (options: InstanceProcessingOptions) =>
      requester.request<InstanceProcessingResult>("instance.stop", options),
    parameters: (options: InstanceParametersOptions) =>
      requester.request<InstanceParametersResult>("instance.parameters", options),
    parameterGet: (options: InstanceParameterGetOptions) =>
      requester.request<InstanceParameterGetResult>("instance.parameter.get", options),
    parameterInfo: (options: InstanceParameterInfoOptions) =>
      requester.request<InstanceParameterInfoResult>("instance.parameter.info", options),
    parameterValueByString: (options: InstanceParameterValueByStringOptions) =>
      requester.request<InstanceParameterValueByStringResult>(
        "instance.parameter.valueByString",
        options,
      ),
    parameterNormalizedByPlain: (options: InstanceParameterNormalizedByPlainOptions) =>
      requester.request<InstanceParameterNormalizedByPlainResult>(
        "instance.parameter.normalizedByPlain",
        options,
      ),
    parameterSet: (options: InstanceParameterSetOptions) =>
      requester.request<InstanceParameterSetResult>("instance.parameter.set", options),
    parameterBeginEdit: (options: InstanceParameterBeginEditOptions) =>
      requester.request<InstanceParameterBeginEditResult>(
        "instance.parameter.beginEdit",
        options,
      ),
    parameterPerformEdit: (options: InstanceParameterPerformEditOptions) =>
      requester.request<InstanceParameterPerformEditResult>(
        "instance.parameter.performEdit",
        options,
      ),
    parameterEndEdit: (options: InstanceParameterEndEditOptions) =>
      requester.request<InstanceParameterEndEditResult>(
        "instance.parameter.endEdit",
        options,
      ),
    parameterEdit: (options: InstanceParameterEditOptions) =>
      requester.request<InstanceParameterEditAggregateResult>(
        "instance.parameter.edit",
        options,
      ),
    units: (options: InstanceUnitsOptions) =>
      requester.request<InstanceUnitsResult>("instance.units", options),
    selectUnit: (options: InstanceSelectUnitOptions) =>
      requester.request<InstanceSelectUnitResult>("instance.selectUnit", options),
    unitByBus: (options: InstanceUnitByBusOptions) =>
      requester.request<InstanceUnitByBusResult>("instance.unitByBus", options),
    setUnitProgramData: (options: InstanceSetUnitProgramDataOptions) =>
      requester.request<InstanceSetUnitProgramDataResult>(
        "instance.setUnitProgramData",
        options,
      ),
    setUnitProgramDataAndRefresh: (
      options: InstanceSetUnitProgramDataAndRefreshOptions,
    ) =>
      requester.request<InstanceSetUnitProgramDataAndRefreshResult>(
        "instance.setUnitProgramDataAndRefresh",
        options,
      ),
    programDataSupported: (options: InstanceProgramDataOptions) =>
      requester.request<InstanceProgramDataSupportedResult>(
        "instance.programData.supported",
        options,
      ),
    getProgramData: (options: InstanceProgramDataOptions) =>
      requester.request<InstanceProgramDataResult>("instance.programData.get", options),
    setProgramData: (options: InstanceSetProgramDataOptions) =>
      requester.request<InstanceSetProgramDataResult>("instance.programData.set", options),
    setProgramDataAndRefresh: (options: InstanceSetProgramDataAndRefreshOptions) =>
      requester.request<InstanceSetProgramDataAndRefreshResult>(
        "instance.programData.setAndRefresh",
        options,
      ),
    unitDataSupported: (options: InstanceUnitDataOptions) =>
      requester.request<InstanceUnitDataSupportedResult>(
        "instance.unitData.supported",
        options,
      ),
    getUnitData: (options: InstanceUnitDataOptions) =>
      requester.request<InstanceUnitDataResult>("instance.unitData.get", options),
    setUnitData: (options: InstanceSetUnitDataOptions) =>
      requester.request<InstanceSetUnitDataResult>("instance.unitData.set", options),
    setUnitDataAndRefresh: (options: InstanceSetUnitDataAndRefreshOptions) =>
      requester.request<InstanceSetUnitDataAndRefreshResult>(
        "instance.unitData.setAndRefresh",
        options,
      ),
    getState: (options: InstanceGetStateOptions) =>
      requester.request<InstanceGetStateResult>("instance.getState", options),
    setState: (options: InstanceSetStateOptions) =>
      requester.request<InstanceSetStateResult>("instance.setState", options),
    setStateAndRefresh: (options: InstanceSetStateAndRefreshOptions) =>
      requester.request<InstanceSetStateAndRefreshResult>(
        "instance.state.setAndRefresh",
        options,
      ),
    notifyComponent: (options: InstanceConnectionNotifyOptions) =>
      requester.request<InstanceConnectionNotifyResult>(
        "instance.connection.notifyComponent",
        options,
      ),
    notifyController: (options: InstanceConnectionNotifyOptions) =>
      requester.request<InstanceConnectionNotifyResult>(
        "instance.connection.notifyController",
        options,
      ),
    notifyComponentAndRefresh: (options: InstanceConnectionNotifyAndRefreshOptions) =>
      requester.request<InstanceConnectionNotifyAndRefreshResult>(
        "instance.connection.notifyComponentAndRefresh",
        options,
      ),
    notifyControllerAndRefresh: (options: InstanceConnectionNotifyAndRefreshOptions) =>
      requester.request<InstanceConnectionNotifyAndRefreshResult>(
        "instance.connection.notifyControllerAndRefresh",
        options,
      ),
    destroy: (options: InstanceDestroyOptions) =>
      requester.request<InstanceDestroyResult>("instance.destroy", options),
    openStream: (options: StreamLifecycleOptions) =>
      requester.request<InstanceDescriptor>("stream.open", options),
    closeStream: (options: StreamLifecycleOptions) =>
      requester.request<InstanceDescriptor>("stream.close", options),
    sharedMemoryCreate: (options: StreamSharedMemoryCreateOptions) =>
      requester.request<StreamSharedMemoryCreateResult>(
        "stream.sharedMemory.create",
        options,
      ),
    sharedMemoryDestroy: (options: StreamLifecycleOptions) =>
      requester.request<StreamSharedMemoryDestroyResult>(
        "stream.sharedMemory.destroy",
        options,
      ),
    sharedMemoryStatus: (options: StreamLifecycleOptions) =>
      requester.request<StreamSharedMemoryStatusResult>(
        "stream.sharedMemory.status",
        options,
      ),
    sharedMemoryProcess: (options: StreamSharedMemoryProcessOptions) =>
      requester.request<StreamSharedMemoryProcessResult>(
        "stream.sharedMemory.process",
        options,
      ),
    sharedMemoryPumpStart: (options: StreamSharedMemoryPumpStartOptions) =>
      requester.request<StreamSharedMemoryPumpStartResult>(
        "stream.sharedMemory.pump.start",
        options,
      ),
    sharedMemoryPumpStop: (options: StreamLifecycleOptions) =>
      requester.request<StreamSharedMemoryPumpStopResult>(
        "stream.sharedMemory.pump.stop",
        options,
      ),
    sharedMemoryPumpStatus: (options: StreamLifecycleOptions) =>
      requester.request<StreamSharedMemoryPumpStatusResult>(
        "stream.sharedMemory.pump.status",
        options,
      ),
    sharedMemoryPumpEnqueueEvents: (
      options: StreamSharedMemoryPumpEnqueueEventsOptions,
    ) =>
      requester.request<StreamSharedMemoryPumpEnqueueEventsResult>(
        "stream.sharedMemory.pump.enqueueEvents",
        options,
      ),
    sharedMemoryPumpClearEvents: (options: StreamLifecycleOptions) =>
      requester.request<StreamSharedMemoryPumpClearEventsResult>(
        "stream.sharedMemory.pump.clearEvents",
        options,
      ),
  };
}

export function createPluginApi(requester: ApiRequester): PluginApi {
  return {
    scan: (options?: PluginScanOptions) =>
      requester.request<PluginScanReport>("plugin.scan", options ?? {}),
    list: (options?: PluginListOptions) =>
      requester.request<PluginScanReport>("plugin.list", options ?? {}),
    factoryInfo: (options: PluginFactoryInfoOptions) =>
      requester.request<PluginFactoryInfo>("plugin.factoryInfo", options),
  };
}
