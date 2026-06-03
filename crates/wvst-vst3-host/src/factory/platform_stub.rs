use std::path::Path;

use super::{Vst3ComponentProbe, Vst3FactoryInfo};
use crate::{HostError, HostResult, Vst3ProcessingConfig};

pub(super) struct LoadedPluginModule;

pub fn load_vst3_factory_info(_bundle_path: &Path) -> HostResult<Vst3FactoryInfo> {
    Err(HostError::UnsupportedPlatform(
        "VST3 factory loading is currently implemented for macOS only",
    ))
}

pub fn create_vst3_component_probe(
    _bundle_path: &Path,
    _class_id: &str,
    _interface_id: &str,
) -> HostResult<Vst3ComponentProbe> {
    Err(HostError::UnsupportedPlatform(
        "VST3 createInstance is currently implemented for macOS only",
    ))
}

pub fn create_vst3_component_instance(
    _bundle_path: &Path,
    _class_id: &str,
    _processing_config: Vst3ProcessingConfig,
) -> HostResult<super::Vst3LoadedComponent> {
    Err(HostError::UnsupportedPlatform(
        "VST3 createInstance is currently implemented for macOS only",
    ))
}
