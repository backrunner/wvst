use std::path::PathBuf;

pub fn default_vst3_paths() -> Vec<PathBuf> {
    platform_vst3_paths()
}

#[cfg(target_os = "macos")]
fn platform_vst3_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/Library/Audio/Plug-Ins/VST3")];

    if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join("Library/Audio/Plug-Ins/VST3"));
    }

    paths
}

#[cfg(target_os = "windows")]
fn platform_vst3_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(program_files) = std::env::var_os("COMMONPROGRAMFILES") {
        paths.push(PathBuf::from(program_files).join("VST3"));
    }

    paths
}

#[cfg(all(unix, not(target_os = "macos")))]
fn platform_vst3_paths() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/usr/lib/vst3"),
        PathBuf::from("/usr/local/lib/vst3"),
    ];

    if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".vst3"));
    }

    paths
}
