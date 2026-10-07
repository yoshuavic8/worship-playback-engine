//! Application data layout. Never relies on the current working directory.
//!
//!   ~/Library/Application Support/app.worshipplayback.engine/
//!     database.sqlite
//!     songs/ (or a custom library location)
//!     setlists/ models/ resources/ cache/ logs/ python-env/

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
    pub fn python_env(&self) -> PathBuf {
        self.root.join("python-env")
    }
    pub fn python_bin(&self) -> PathBuf {
        self.python_env().join("bin").join("python")
    }
}

/// Locate an executable even when launched from Finder (where PATH is minimal).
pub fn find_tool(name: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut dirs: Vec<PathBuf> = vec![
        "/opt/homebrew/bin".into(),
        "/usr/local/bin".into(),
        "/usr/bin".into(),
        "/bin".into(),
        Path::new(&home).join(".local/bin"),
        Path::new(&home).join(".cargo/bin"),
    ];
    if let Ok(path) = std::env::var("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// Directory containing the Python audio worker project.
pub fn worker_dir(resource_dir: Option<PathBuf>) -> PathBuf {
    if let Some(r) = resource_dir {
        let p = r.join("audio-worker");
        if p.join("pyproject.toml").exists() && !cfg!(debug_assertions) {
            return p;
        }
    }
    // development checkout: apps/desktop/src-tauri → repo root
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../audio-worker")
}
