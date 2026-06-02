pub mod audio_processor;
pub mod error;
pub mod factory;
pub mod lifecycle;
pub mod metadata;
pub mod module;
pub mod process_buffers;
pub mod processor;
mod vst3_abi;

pub use audio_processor::Vst3AudioProcessor;
pub use error::{HostError, HostResult};
pub use factory::{
    Vst3ComponentProbe, Vst3FactoryClass, Vst3FactoryInfo, create_vst3_component_probe,
    load_vst3_factory_info,
};
pub use lifecycle::{Vst3Lifecycle, Vst3LifecycleState, Vst3ProcessingConfig};
pub use metadata::HeadlessPluginMetadata;
pub use module::{Vst3ModuleProbe, Vst3ModuleSymbols, find_vst3_executable, probe_vst3_module};
pub use process_buffers::Vst3ProcessBuffers;
pub use processor::{HeadlessPluginInstance, ProcessStats};
