use std::path::{Path, PathBuf};

use libloading::Library;
use serde::{Deserialize, Serialize};

use crate::{HostError, HostResult};

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ModuleProbe {
    pub bundle_path: String,
    pub executable_path: String,
    pub symbols: Vst3ModuleSymbols,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vst3ModuleSymbols {
    pub get_plugin_factory: bool,
    pub entry: bool,
    pub exit: bool,
    pub entry_symbol: Option<String>,
    pub exit_symbol: Option<String>,
}

pub fn probe_vst3_module(bundle_path: impl AsRef<Path>) -> HostResult<Vst3ModuleProbe> {
    let bundle_path = bundle_path.as_ref();
    let executable_path = find_vst3_executable(bundle_path)?;
    let symbols = probe_symbols(&executable_path)?;

    Ok(Vst3ModuleProbe {
        bundle_path: path_to_string(bundle_path),
        executable_path: path_to_string(&executable_path),
        symbols,
    })
}

pub fn find_vst3_executable(bundle_path: impl AsRef<Path>) -> HostResult<PathBuf> {
    let bundle_path = bundle_path.as_ref();
    let candidates = executable_candidates(bundle_path);

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    Err(HostError::BundleExecutableNotFound(path_to_string(
        bundle_path,
    )))
}

fn executable_candidates(bundle_path: &Path) -> Vec<PathBuf> {
    let stem = bundle_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    platform_executable_candidates(bundle_path, stem)
}

#[cfg(target_os = "macos")]
fn platform_executable_candidates(bundle_path: &Path, stem: &str) -> Vec<PathBuf> {
    let macos_dir = bundle_path.join("Contents").join("MacOS");
    let mut candidates = Vec::new();

    if !stem.is_empty() {
        candidates.push(macos_dir.join(stem));
    }

    candidates.extend(list_regular_files(&macos_dir));
    candidates
}

#[cfg(target_os = "windows")]
fn platform_executable_candidates(bundle_path: &Path, stem: &str) -> Vec<PathBuf> {
    let module_dir = bundle_path.join("Contents").join("x86_64-win");
    let mut candidates = Vec::new();

    if !stem.is_empty() {
        candidates.push(module_dir.join(format!("{stem}.vst3")));
    }

    candidates.extend(list_regular_files(&module_dir));
    candidates
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_executable_candidates(bundle_path: &Path, stem: &str) -> Vec<PathBuf> {
    let module_dir = bundle_path.join("Contents").join("x86_64-linux");
    let mut candidates = Vec::new();

    if !stem.is_empty() {
        candidates.push(module_dir.join(format!("{stem}.so")));
    }

    candidates.extend(list_regular_files(&module_dir));
    candidates
}

fn list_regular_files(directory: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(directory)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_file())
                .collect()
        })
        .unwrap_or_default()
}

fn probe_symbols(executable_path: &Path) -> HostResult<Vst3ModuleSymbols> {
    // SAFETY: Loading arbitrary VST3 modules is intentionally isolated to this
    // crate and worker process. This function only checks symbol presence and
    // does not call plugin entry points or factory functions.
    let library = unsafe { Library::new(executable_path) }
        .map_err(|error| HostError::ModuleLoadFailed(error.to_string()))?;

    Ok(Vst3ModuleSymbols {
        get_plugin_factory: has_symbol(&library, b"GetPluginFactory\0"),
        entry: platform_entry_symbol().is_some_and(|symbol| has_symbol(&library, symbol)),
        exit: platform_exit_symbol().is_some_and(|symbol| has_symbol(&library, symbol)),
        entry_symbol: platform_entry_symbol_name().map(str::to_string),
        exit_symbol: platform_exit_symbol_name().map(str::to_string),
    })
}

fn has_symbol(library: &Library, symbol: &[u8]) -> bool {
    // SAFETY: The symbol is never invoked. `libloading` validates lookup for the
    // lifetime of `library`, and the returned pointer is immediately discarded.
    unsafe { library.get::<*mut std::ffi::c_void>(symbol) }.is_ok()
}

#[cfg(target_os = "macos")]
fn platform_entry_symbol() -> Option<&'static [u8]> {
    Some(b"bundleEntry\0")
}

#[cfg(target_os = "macos")]
fn platform_exit_symbol() -> Option<&'static [u8]> {
    Some(b"bundleExit\0")
}

#[cfg(target_os = "macos")]
fn platform_entry_symbol_name() -> Option<&'static str> {
    Some("bundleEntry")
}

#[cfg(target_os = "macos")]
fn platform_exit_symbol_name() -> Option<&'static str> {
    Some("bundleExit")
}

#[cfg(target_os = "windows")]
fn platform_entry_symbol() -> Option<&'static [u8]> {
    Some(b"InitDll\0")
}

#[cfg(target_os = "windows")]
fn platform_exit_symbol() -> Option<&'static [u8]> {
    Some(b"ExitDll\0")
}

#[cfg(target_os = "windows")]
fn platform_entry_symbol_name() -> Option<&'static str> {
    Some("InitDll")
}

#[cfg(target_os = "windows")]
fn platform_exit_symbol_name() -> Option<&'static str> {
    Some("ExitDll")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_entry_symbol() -> Option<&'static [u8]> {
    Some(b"ModuleEntry\0")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_exit_symbol() -> Option<&'static [u8]> {
    Some(b"ModuleExit\0")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_entry_symbol_name() -> Option<&'static str> {
    Some("ModuleEntry")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_exit_symbol_name() -> Option<&'static str> {
    Some("ModuleExit")
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{create_dir_all, remove_dir_all, write};

    #[test]
    fn finds_platform_executable_candidate() {
        let root = test_root("executable");
        let bundle = root.join("Echo.vst3");
        let executable = platform_test_executable(&bundle);
        create_dir_all(executable.parent().expect("parent")).expect("bundle directory");
        write(&executable, b"not a real dylib").expect("probe file");

        assert_eq!(find_vst3_executable(&bundle), Ok(executable));

        remove_dir_all(root).expect("cleanup");
    }

    #[cfg(target_os = "macos")]
    fn platform_test_executable(bundle: &Path) -> PathBuf {
        bundle.join("Contents").join("MacOS").join("Echo")
    }

    #[cfg(target_os = "windows")]
    fn platform_test_executable(bundle: &Path) -> PathBuf {
        bundle.join("Contents").join("x86_64-win").join("Echo.vst3")
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn platform_test_executable(bundle: &Path) -> PathBuf {
        bundle.join("Contents").join("x86_64-linux").join("Echo.so")
    }

    fn test_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("wvst-vst3-host-{name}-{}", std::process::id()));
        let _ = remove_dir_all(&root);
        root
    }
}
