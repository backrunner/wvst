use std::fs;
use std::hash::Hasher;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ScanError;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginFormat {
    Vst3,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MetadataSource {
    ModuleInfo,
    BundleName,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginClass {
    pub class_id: Option<String>,
    pub name: String,
    pub category: Option<String>,
    pub subcategories: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginDescriptor {
    pub plugin_id: String,
    pub format: PluginFormat,
    pub name: String,
    pub vendor: Option<String>,
    pub version: Option<String>,
    pub path: String,
    pub classes: Vec<PluginClass>,
    pub metadata_source: MetadataSource,
}

pub fn parse_vst3_bundle(path: impl AsRef<Path>) -> Result<PluginDescriptor, ScanError> {
    let path = path.as_ref();
    if !path.is_dir() {
        return Err(ScanError::NotDirectory(path.to_path_buf()));
    }

    let module_info_path = path.join("Contents").join("moduleinfo.json");
    if !module_info_path.exists() {
        return Ok(descriptor_from_bundle_name(path));
    }

    let contents = fs::read_to_string(&module_info_path).map_err(|error| ScanError::Io {
        path: module_info_path.clone(),
        message: error.to_string(),
    })?;
    let json = serde_json::from_str::<Value>(&contents).map_err(|error| ScanError::Json {
        path: module_info_path,
        message: error.to_string(),
    })?;

    Ok(descriptor_from_module_info(path, &json))
}

fn descriptor_from_module_info(path: &Path, json: &Value) -> PluginDescriptor {
    let fallback_name = bundle_name(path);
    let name = first_string(json, &["Name", "name", "Plugin Name"]).unwrap_or(fallback_name);
    let vendor = first_string(json, &["Vendor", "vendor", "Company", "Manufacturer"]);
    let version = first_string(json, &["Version", "version"]);
    let classes = parse_classes(json).unwrap_or_else(|| vec![default_class(name.clone())]);

    PluginDescriptor {
        plugin_id: plugin_id_for_path(path),
        format: PluginFormat::Vst3,
        name,
        vendor,
        version,
        path: path_to_string(path),
        classes,
        metadata_source: MetadataSource::ModuleInfo,
    }
}

fn descriptor_from_bundle_name(path: &Path) -> PluginDescriptor {
    let name = bundle_name(path);

    PluginDescriptor {
        plugin_id: plugin_id_for_path(path),
        format: PluginFormat::Vst3,
        name: name.clone(),
        vendor: None,
        version: None,
        path: path_to_string(path),
        classes: vec![default_class(name)],
        metadata_source: MetadataSource::BundleName,
    }
}

fn parse_classes(json: &Value) -> Option<Vec<PluginClass>> {
    let classes = first_array(json, &["Classes", "classes"])?;
    let parsed = classes
        .iter()
        .filter_map(parse_class)
        .collect::<Vec<PluginClass>>();

    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

fn parse_class(value: &Value) -> Option<PluginClass> {
    let name = first_string(value, &["Name", "name"])?;

    Some(PluginClass {
        class_id: first_string(value, &["CID", "Class ID", "classId", "cid"]),
        name,
        category: first_string(value, &["Category", "category"]),
        subcategories: parse_subcategories(value),
    })
}

fn parse_subcategories(value: &Value) -> Vec<String> {
    if let Some(items) = first_array(value, &["Sub Categories", "SubCategories", "subcategories"]) {
        return items
            .iter()
            .filter_map(Value::as_str)
            .filter_map(non_empty_string)
            .collect();
    }

    first_string(value, &["Sub Categories", "SubCategories", "subcategories"])
        .map(|value| {
            value
                .split(['|', ','])
                .filter_map(non_empty_string)
                .collect()
        })
        .unwrap_or_default()
}

fn default_class(name: String) -> PluginClass {
    PluginClass {
        class_id: None,
        name,
        category: None,
        subcategories: Vec::new(),
    }
}

fn first_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .filter_map(Value::as_str)
        .find_map(non_empty_string)
}

fn first_array<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Vec<Value>> {
    keys.iter()
        .filter_map(|key| value.get(*key))
        .find_map(Value::as_array)
}

fn non_empty_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn bundle_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Unknown VST3")
        .to_string()
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn plugin_id_for_path(path: &Path) -> String {
    let mut hasher = Fnv1a64::default();
    hasher.write(path_to_string(path).as_bytes());
    format!("vst3:{:016x}", hasher.finish())
}

#[derive(Default)]
struct Fnv1a64(u64);

impl Hasher for Fnv1a64 {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        if self.0 == 0 {
            self.0 = 0xcbf29ce484222325;
        }

        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x100000001b3);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, remove_dir_all, write};
    use std::path::PathBuf;

    #[test]
    fn parses_moduleinfo_metadata() {
        let root = test_root("metadata");
        let bundle = root.join("Acme.vst3");
        create_dir_all(bundle.join("Contents")).expect("bundle directory");
        write(
            bundle.join("Contents/moduleinfo.json"),
            r#"{"Name":"Acme EQ","Vendor":"Acme","Version":"1.2.3","Classes":[{"CID":"abc","Name":"Acme EQ","Category":"Fx","Sub Categories":"EQ|Stereo"}]}"#,
        )
        .expect("moduleinfo");

        let descriptor = parse_vst3_bundle(&bundle).expect("descriptor");
        assert_eq!(descriptor.name, "Acme EQ");
        assert_eq!(descriptor.vendor.as_deref(), Some("Acme"));
        assert_eq!(descriptor.classes[0].subcategories, ["EQ", "Stereo"]);

        remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn falls_back_to_bundle_name() {
        let root = test_root("fallback");
        let bundle = root.join("Synth.vst3");
        create_dir_all(&bundle).expect("bundle directory");

        let descriptor = parse_vst3_bundle(&bundle).expect("descriptor");
        assert_eq!(descriptor.name, "Synth");
        assert_eq!(descriptor.metadata_source, MetadataSource::BundleName);

        remove_dir_all(root).expect("cleanup");
    }

    fn test_root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("wvst-scanner-{name}-{}", std::process::id()));
        let _ = remove_dir_all(&root);
        root
    }
}
