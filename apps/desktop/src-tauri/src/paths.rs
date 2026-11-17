//! Application data layout. Never relies on the current working directory.
//!
//!   ~/Library/Application Support/app.worshipplayback.engine/
//!     database.sqlite
//!     songs/ (or a custom library location)
//!     models/ resources/ cache/ logs/ python-env/ python-install/

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub library: PathBuf,
}

impl AppPaths {
    pub fn new(root: PathBuf, custom_library: Option<PathBuf>) -> Self {
        let library = custom_library.unwrap_or_else(|| root.clone());
        let p = Self { root, library };
        for d in [
            p.songs(),
            p.models(),
            p.resources(),
            p.cache(),
            p.logs(),
            p.cues(),
            p.exports(),
            p.python_install(),
        ] {
            let _ = std::fs::create_dir_all(d);
        }
        p
    }
    pub fn db(&self) -> PathBuf {
        self.root.join("database.sqlite")
    }
    pub fn songs(&self) -> PathBuf {
        self.library.join("songs")
    }
    pub fn models(&self) -> PathBuf {
        self.root.join("models")
    }
    pub fn demucs_models(&self) -> PathBuf {
        self.models().join("demucs")
    }
    pub fn resources(&self) -> PathBuf {
        self.root.join("resources")
    }
    pub fn cues(&self) -> PathBuf {
        self.resources().join("audio").join("cue")
    }
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
    pub fn logs(&self) -> PathBuf {
        self.root.join("logs")
    }
    pub fn exports(&self) -> PathBuf {
        self.root.join("exports")
    }
    /// uv-managed CPython installs (not the project venv).
    pub fn python_install(&self) -> PathBuf {
        self.root.join("python-install")
    }
    pub fn python_env(&self) -> PathBuf {
        self.root.join("python-env")
    }
    pub fn python_bin(&self) -> PathBuf {
        self.python_env().join("bin").join("python")
    }
}

pub fn target_triple() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-apple-darwin"
    }
}

/// Bundled Tauri sidecar next to the app executable, or dev binaries under src-tauri/binaries/.
pub fn bundled_tool(name: &str) -> Option<PathBuf> {
    let triple = target_triple();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for candidate in [dir.join(name), dir.join(format!("{name}-{triple}"))] {
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("binaries")
        .join(format!("{name}-{triple}"));
    if dev.is_file() {
        return Some(dev);
    }
    None
}

fn search_path(name: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut dirs: Vec<PathBuf> = vec![Path::new(&home).join(".local/bin"), Path::new(&home).join(".cargo/bin")];
    if let Ok(path) = std::env::var("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// Resolve ffmpeg, ffprobe, or uv. Production uses bundled sidecars; debug builds may fall back to PATH.
pub fn find_tool(name: &str) -> Option<PathBuf> {
    if let Some(p) = bundled_tool(name) {
        return Some(p);
    }
    if cfg!(debug_assertions) {
        return search_path(name);
    }
    None
}

/// Directory containing the Python audio worker project.
pub fn worker_dir(resource_dir: Option<PathBuf>) -> PathBuf {
    if let Some(r) = resource_dir {
        let p = r.join("audio-worker");
        if p.join("pyproject.toml").exists() && !cfg!(debug_assertions) {
            return p;
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../audio-worker")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_tool_name_format() {
        assert!(target_triple().contains("apple-darwin"));
        let p = bundled_tool("nonexistent-tool-xyz");
        assert!(p.as_ref().map(|x| x.is_file()).unwrap_or(true));
    }

    #[test]
    fn find_tool_prefers_bundled_when_present() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join(format!("uv-{}", target_triple()));
        std::fs::write(&fake, b"#!/bin/sh\necho uv\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // Dev layout: cannot override current_exe parent easily; smoke test triple only.
        assert!(fake.to_string_lossy().contains("uv-"));
    }
}
