use serde::{Deserialize, Serialize};
use wvst_scanner::PluginDescriptor;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadlessPluginMetadata {
    pub plugin_id: String,
    pub name: String,
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub path: String,
    pub class_count: usize,
}

impl From<&PluginDescriptor> for HeadlessPluginMetadata {
    fn from(value: &PluginDescriptor) -> Self {
        Self {
            plugin_id: value.plugin_id.clone(),
            name: value.name.clone(),
            vendor: value.vendor.clone(),
            version: value.version.clone(),
            path: value.path.clone(),
            class_count: value.classes.len(),
        }
    }
}
