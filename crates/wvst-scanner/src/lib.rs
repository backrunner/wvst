pub mod error;
pub mod metadata;
pub mod paths;
pub mod scan;

pub use error::ScanError;
pub use metadata::{
    MetadataSource, PluginClass, PluginDescriptor, PluginFormat, parse_vst3_bundle,
};
pub use paths::default_vst3_paths;
pub use scan::{ScanFailure, ScanReport, scan_paths};
