export interface PluginClass {
  classId?: string;
  name: string;
  category?: string;
  subcategories: string[];
}

export interface PluginDescriptor {
  pluginId: string;
  format: "vst3";
  name: string;
  vendor?: string;
  version?: string;
  path: string;
  classes: PluginClass[];
  metadataSource: "module-info" | "bundle-name";
}

export interface PluginScanFailure {
  path: string;
  reason: string;
}

export interface PluginScanReport {
  plugins: PluginDescriptor[];
  failures: PluginScanFailure[];
}

export interface PluginFactoryClass {
  classId: string;
  cardinality: number;
  category?: string;
  name?: string;
}

export interface PluginFactoryInfo {
  vendor?: string;
  url?: string;
  email?: string;
  flags: number;
  classes: PluginFactoryClass[];
}

export interface PluginScanOptions {
  paths?: string[];
}

export interface PluginListOptions extends PluginScanOptions {
  rescan?: boolean;
}

export interface PluginFactoryInfoOptions {
  path: string;
}

export interface PluginApi {
  scan(options?: PluginScanOptions): Promise<PluginScanReport>;
  list(options?: PluginListOptions): Promise<PluginScanReport>;
  factoryInfo(options: PluginFactoryInfoOptions): Promise<PluginFactoryInfo>;
}
