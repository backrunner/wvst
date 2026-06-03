export interface InstanceParametersOptions {
  instanceId: number;
}

export interface Vst3ParameterInfo {
  id: number;
  title: string | null;
  shortTitle: string | null;
  units: string | null;
  stepCount: number;
  defaultNormalizedValue: number;
  unitId: number;
  flags: Vst3ParameterFlags;
}

export interface Vst3ParameterFlags {
  raw: number;
  canAutomate: boolean;
  readOnly: boolean;
  wrapAround: boolean;
  list: boolean;
  hidden: boolean;
  programChange: boolean;
  bypass: boolean;
}

export interface InstanceParametersResult {
  instanceId: number;
  parameters: Vst3ParameterInfo[];
}

export interface InstanceParameterGetOptions {
  instanceId: number;
  parameterId: number;
}

export interface InstanceParameterGetResult {
  instanceId: number;
  parameterId: number;
  valueNormalized: number;
}

export interface InstanceParameterInfoOptions extends InstanceParameterGetOptions {
  valueNormalized?: number;
}

export interface InstanceParameterInfoResult extends InstanceParameterGetResult {
  valuePlain: number | null;
  valueString: string | null;
}

export interface InstanceParameterValueByStringOptions
  extends InstanceParameterGetOptions {
  value: string;
}

export type InstanceParameterValueByStringResult = InstanceParameterInfoResult;

export interface InstanceParameterNormalizedByPlainOptions
  extends InstanceParameterGetOptions {
  valuePlain: number;
}

export type InstanceParameterNormalizedByPlainResult = InstanceParameterInfoResult;

export interface InstanceParameterSetOptions extends InstanceParameterGetOptions {
  valueNormalized: number;
}

export type InstanceParameterSetResult = InstanceParameterGetResult;

export type InstanceParameterBeginEditOptions = InstanceParameterGetOptions;

export interface InstanceParameterEditResult {
  instanceId: number;
  parameterId: number;
  editKind: "begin-edit" | "perform-edit" | "end-edit";
  valueNormalized: number | null;
}

export interface InstanceParameterPerformEditOptions
  extends InstanceParameterGetOptions {
  valueNormalized: number;
}

export type InstanceParameterBeginEditResult = InstanceParameterEditResult;
export type InstanceParameterPerformEditResult = InstanceParameterEditResult;
export type InstanceParameterEndEditOptions = InstanceParameterGetOptions;
export type InstanceParameterEndEditResult = InstanceParameterEditResult;

export interface InstanceParameterEditOptions extends InstanceParameterSetOptions {}

export interface InstanceParameterEditAggregateResult
  extends InstanceParameterSetResult {
  beginEdit: InstanceParameterBeginEditResult | null;
  performEdit: InstanceParameterPerformEditResult | null;
  endEdit: InstanceParameterEndEditResult | null;
}
