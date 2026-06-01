pub mod error;
pub mod factory;
pub mod metadata;
pub mod module;
pub mod processor;
mod vst3_abi;

pub use error::{HostError, HostResult};
pub use factory::{Vst3FactoryClass, Vst3FactoryInfo, load_vst3_factory_info};
pub use metadata::HeadlessPluginMetadata;
pub use module::{Vst3ModuleProbe, Vst3ModuleSymbols, find_vst3_executable, probe_vst3_module};
pub use processor::{HeadlessPluginInstance, ProcessStats};
