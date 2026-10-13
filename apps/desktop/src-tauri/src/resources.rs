//! Resource / Model Manager. Every downloadable resource has a name, version, source URL,
//! license, checksum and size. Nothing is ever downloaded silently and downloaded binaries are
//! never executed (model weights are data, the Python environment is built by `uv` from a lockfile).

use crate::core::Ctx;
use crate::logs::{self, cat};
use crate::paths::{find_tool, AppPaths};
use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tauri::Emitter;

#[derive(Debug, Clone)]
pub enum Checksum {
    /// Demucs convention: the file name carries the first hex chars of the SHA-256.
    Sha256Prefix(&'static str),
    Sha256(&'static str),
}

#[derive(Debug, Clone)]
pub struct FileDef {
    pub url: &'static str,
    pub filename: &'static str,
    pub size: u64,
    pub checksum: Checksum,
}

#[derive(Debug, Clone)]
pub struct ResourceDef {
    pub id: &'static str,
    pub kind: &'static str,
    /// "demucs" | "demucs-mlx" | "mdx"
    pub engine: &'static str,
    pub model: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub version: &'static str,
    pub license: &'static str,
    pub source: &'static str,
    pub recommended: bool,
    pub files: Vec<FileDef>,
    /// Extra files written after download (e.g. demucs bag YAML).
    pub extra: Vec<(&'static str, &'static str)>,
}

const FB: &str = "https://dl.fbaipublicfiles.com/demucs/hybrid_transformer/";

pub fn catalog() -> Vec<ResourceDef> {
    let th = |name: &'static str, url: &'static str, prefix: &'static str| FileDef {
        url,
        filename: name,
        size: if name.starts_with("955717e8") { 84_141_911 } else { 84_141_271 },
        checksum: Checksum::Sha256Prefix(prefix),
    };
    let _ = FB;
    vec![
        ResourceDef {
            id: "demucs-mlx-htdemucs",
            kind: "model",
            engine: "demucs-mlx",
            model: "htdemucs",
            name: "Demucs HT (MLX, Apple Silicon)",
            description: "Hybrid Transformer Demucs converted for Apple's MLX framework. Fastest option on M-series Macs.",
            version: "htdemucs / demucs-mlx 1.6",
            license: "MIT (Meta Platforms; MLX port by ssmall256)",
            source: "https://huggingface.co/ssmall256/demucs-mlx",
            recommended: true,
            files: vec![
                FileDef {
                    url: "https://huggingface.co/ssmall256/demucs-mlx/resolve/main/htdemucs_config.json",
                    filename: "htdemucs_config.json",
                    size: 4215,
                    checksum: Checksum::Sha256("23657b19db14771aecf366ceedbf846c1386836c10043c97f4569acf05526b78"),
                },
                FileDef {
                    url: "https://huggingface.co/ssmall256/demucs-mlx/resolve/main/htdemucs.safetensors",
                    filename: "htdemucs.safetensors",
                    size: 168_005_865,
                    checksum: Checksum::Sha256("339d267a7a6983a11eedbdc00413c602a65e9b9103f695fb5c2b2a481cd9d297"),
                },
            ],
            extra: vec![],
        },
        ResourceDef {
            id: "demucs-htdemucs",
            kind: "model",
            engine: "demucs",
            model: "htdemucs",
            name: "Demucs HT (PyTorch)",
            description: "Official Meta Hybrid Transformer Demucs v4. Runs on the Apple GPU (MPS) or CPU. Reference implementation / fallback.",
            version: "htdemucs (955717e8)",
            license: "MIT (Meta Platforms, Inc.)",
            source: "https://github.com/adefossez/demucs",
            recommended: true,
            files: vec![th(
                "955717e8-8726e21a.th",
                "https://dl.fbaipublicfiles.com/demucs/hybrid_transformer/955717e8-8726e21a.th",
                "8726e21a",
            )],
            extra: vec![("htdemucs.yaml", "models: ['955717e8']\n")],
        },
        ResourceDef {
            id: "demucs-htdemucs_ft",
            kind: "model",
            engine: "demucs",
            model: "htdemucs_ft",
            name: "Demucs HT Fine-tuned (PyTorch)",
            description: "Bag of 4 fine-tuned models. Slightly cleaner stems, about 4× slower. For songs where quality matters most.",
            version: "htdemucs_ft",
            license: "MIT (Meta Platforms, Inc.)",
            source: "https://github.com/adefossez/demucs",
            recommended: false,
            files: vec![
                th("f7e0c4bc-ba3fe64a.th", "https://dl.fbaipublicfiles.com/demucs/hybrid_transformer/f7e0c4bc-ba3fe64a.th", "ba3fe64a"),
                th("d12395a8-e57c48e6.th", "https://dl.fbaipublicfiles.com/demucs/hybrid_transformer/d12395a8-e57c48e6.th", "e57c48e6"),
                th("92cfc3b6-ef3bcb9c.th", "https://dl.fbaipublicfiles.com/demucs/hybrid_transformer/92cfc3b6-ef3bcb9c.th", "ef3bcb9c"),
                th("04573f0d-f3cf25b2.th", "https://dl.fbaipublicfiles.com/demucs/hybrid_transformer/04573f0d-f3cf25b2.th", "f3cf25b2"),
            ],
            extra: vec![(
                "htdemucs_ft.yaml",
                "models: ['f7e0c4bc', 'd12395a8', '92cfc3b6', '04573f0d']\nweights: [\n  [1., 0., 0., 0.],\n  [0., 1., 0., 0.],\n  [0., 0., 1., 0.],\n  [0., 0., 0., 1.],\n]\n",
            )],
        },
        ResourceDef {
            id: "demucs-mlx-htdemucs_ft",
            kind: "model",
            engine: "demucs-mlx",
            model: "htdemucs_ft",
            name: "Demucs HT Fine-tuned (MLX)",
            description: "Fine-tuned bag of models for MLX. Best quality on Apple Silicon, slower than HT.",
            version: "htdemucs_ft / demucs-mlx 1.6",
            license: "MIT (Meta Platforms; MLX port by ssmall256)",
            source: "https://huggingface.co/ssmall256/demucs-mlx",
            recommended: false,
            files: vec![
                FileDef {
                    url: "https://huggingface.co/ssmall256/demucs-mlx/resolve/main/htdemucs_ft_config.json",
                    filename: "htdemucs_ft_config.json",
                    size: 10253,
                    checksum: Checksum::Sha256("6708535a698d4d4dd285a6f0a80d9d3c228fdd6514e53bdf1ddbc2ee9b0fa872"),
                },
                FileDef {
                    url: "https://huggingface.co/ssmall256/demucs-mlx/resolve/main/htdemucs_ft.safetensors",
                    filename: "htdemucs_ft.safetensors",
                    size: 672_024_519,
                    checksum: Checksum::Sha256("53f03b1ad4b4d211025a35da65460ba61a17547adf9c0544cad0ebcc8d7bbabb"),
                },
            ],
            extra: vec![],
        },
        ResourceDef {
            id: "mdx-inst-hq-3",
            kind: "model",
            engine: "mdx",
            model: "inst-hq-3",
            name: "UVR-MDX-NET Inst HQ 3",
            description: "Vocal remover used by the Worship profile. Demucs still splits drums, bass and other from the instrumental. Best on live recordings where Demucs leaves the vocal in the backing.",
            version: "Inst HQ 3",
            license: "Community model distributed with Ultimate Vocal Remover",
            source: "https://github.com/TRvlvr/model_repo/releases/tag/all_public_uvr_models",
            recommended: true,
            files: vec![FileDef {
                url: "https://github.com/TRvlvr/model_repo/releases/download/all_public_uvr_models/UVR-MDX-NET-Inst_HQ_3.onnx",
                filename: "UVR-MDX-NET-Inst_HQ_3.onnx",
                size: 66_759_214,
                checksum: Checksum::Sha256("317554b07fe1ea5279a77f2b1520a41ea4b93432560c4ffd08792c30fddf9adc"),
            }],
            extra: vec![],
        },
    ]
}

pub fn resource_dir(paths: &AppPaths, def: &ResourceDef) -> PathBuf {
    match def.engine {
        "demucs-mlx" => paths.models().join("demucs-mlx"),
        "mdx" => paths.models().join("mdx"),
        _ => paths.demucs_models(),
    }
}

fn file_ok(dir: &Path, f: &FileDef) -> bool {
    std::fs::metadata(dir.join(f.filename)).map(|m| m.len() == f.size).unwrap_or(false)
}

pub fn is_installed(paths: &AppPaths, def: &ResourceDef) -> bool {
    let dir = resource_dir(paths, def);
    def.files.iter().all(|f| file_ok(&dir, f)) && def.extra.iter().all(|(n, _)| dir.join(n).exists())
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub downloaded: u64,
    pub total: u64,
    pub message: String,
    pub error: Option<String>,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceStatus {
    pub id: String,
    pub kind: String,
    pub engine: String,
    pub model: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub license: String,
    pub source: String,
    pub recommended: bool,
    pub size_bytes: u64,
    pub installed: bool,
    pub path: String,
    pub progress: Option<Progress>,
}

pub struct ResourceManager {
    progress: Mutex<HashMap<String, Progress>>,
    cancel: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceManager {
    pub fn new() -> Self {
        Self { progress: Mutex::new(HashMap::new()), cancel: Mutex::new(HashMap::new()) }
    }

    pub fn status(&self, paths: &AppPaths) -> Vec<ResourceStatus> {
        let prog = self.progress.lock();
        catalog()
            .into_iter()
            .map(|d| ResourceStatus {
                id: d.id.into(),
                kind: d.kind.into(),
                engine: d.engine.into(),
                model: d.model.into(),
                name: d.name.into(),
                description: d.description.into(),
                version: d.version.into(),
                license: d.license.into(),
                source: d.source.into(),
                recommended: d.recommended,
                size_bytes: d.files.iter().map(|f| f.size).sum(),
                installed: is_installed(paths, &d),
                path: resource_dir(paths, &d).to_string_lossy().into(),
                progress: prog.get(d.id).cloned(),
            })
            .collect()
    }

    /// Is a separation model usable for `model` with the configured engine preference?
    pub fn model_installed(&self, paths: &AppPaths, model: &str) -> bool {
        catalog().iter().any(|d| d.model == model && is_installed(paths, d))
    }

    pub fn installed_engines(&self, paths: &AppPaths, model: &str) -> Vec<String> {
        catalog()
            .iter()
            .filter(|d| d.model == model && is_installed(paths, d))
            .map(|d| d.engine.to_string())
            .collect()
    }

    pub(crate) fn set_progress(&self, ctx: &Ctx, id: &str, p: Progress) {
        self.progress.lock().insert(id.into(), p.clone());
        let _ = ctx.app.emit("resource:progress", serde_json::json!({ "id": id, "progress": p }));
    }

    pub fn download(&self, ctx: Arc<Ctx>, id: &str) -> Result<()> {
        let def = catalog().into_iter().find(|d| d.id == id).ok_or_else(|| anyhow!("Unknown resource"))?;
        if self.progress.lock().get(id).map(|p| p.active).unwrap_or(false) {
            return Err(anyhow!("Already downloading"));
        }
        let flag = Arc::new(AtomicBool::new(false));
        self.cancel.lock().insert(id.into(), flag.clone());
        let id = id.to_string();
        std::thread::spawn(move || {
            let rm = &ctx.resources;
            let total: u64 = def.files.iter().map(|f| f.size).sum();
            rm.set_progress(&ctx, &id, Progress { total, active: true, message: "Starting download".into(), ..Default::default() });
            logs::info(cat::SYSTEM, format!("Downloading resource {} from {}", def.name, def.source));
            match download_resource(&ctx, &def, &flag, |downloaded, msg| {
                rm.set_progress(&ctx, &id, Progress { downloaded, total, message: msg, active: true, error: None });
            }) {
                Ok(()) => {
                    let paths = ctx.paths.read().clone();
                    let dir = resource_dir(&paths, &def);
                    let _ = ctx.db.set_resource(
                        def.id,
                        def.kind,
                        def.name,
                        def.version,
                        "installed",
                        total,
                        Some(&dir.to_string_lossy()),
                        Some(&serde_json::json!({ "license": def.license, "source": def.source, "downloadedAt": crate::models::now_iso() })),
                    );
                    rm.set_progress(&ctx, &id, Progress { downloaded: total, total, message: "Installed and verified".into(), active: false, error: None });
                    logs::info(cat::SYSTEM, format!("Installed {}", def.name));
                }
                Err(e) => {
                    let cancelled = flag.load(Ordering::Relaxed);
                    let msg = if cancelled { "Download cancelled".to_string() } else { format!("{e:#}") };
                    rm.set_progress(&ctx, &id, Progress { downloaded: 0, total, message: msg.clone(), active: false, error: (!cancelled).then_some(msg.clone()) });
                    logs::error(cat::SYSTEM, format!("Download of {} failed: {msg}", def.name));
                }
            }
            let _ = ctx.app.emit("resource:changed", &id);
        });
        Ok(())
    }

    pub fn cancel(&self, id: &str) {
        if let Some(f) = self.cancel.lock().get(id) {
            f.store(true, Ordering::Relaxed);
        }
    }

    pub fn remove(&self, ctx: &Ctx, id: &str) -> Result<()> {
        let def = catalog().into_iter().find(|d| d.id == id).ok_or_else(|| anyhow!("Unknown resource"))?;
        let paths = ctx.paths.read().clone();
        let dir = resource_dir(&paths, &def);
        for f in &def.files {
            let _ = std::fs::remove_file(dir.join(f.filename));
            let _ = std::fs::remove_file(dir.join(format!("{}.part", f.filename)));
        }
        for (n, _) in &def.extra {
            let _ = std::fs::remove_file(dir.join(n));
        }
        ctx.db.remove_resource(id)?;
        self.progress.lock().remove(id);
        logs::info(cat::SYSTEM, format!("Removed {}", def.name));
        Ok(())
    }
}

fn verify(path: &Path, c: &Checksum) -> Result<()> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let hex = hex::encode(h.finalize());
    let ok = match c {
        Checksum::Sha256Prefix(p) => hex.starts_with(p),
        Checksum::Sha256(full) => hex == *full,
    };
    if ok {
        Ok(())
    } else {
        Err(anyhow!("Checksum mismatch for {} — the download is corrupt and was discarded", path.display()))
    }
}

fn download_resource(
    ctx: &Ctx,
    def: &ResourceDef,
    cancel: &AtomicBool,
    progress: impl Fn(u64, String),
) -> Result<()> {
    let paths = ctx.paths.read().clone();
    let dir = resource_dir(&paths, def);
    std::fs::create_dir_all(&dir)?;
    let client = reqwest::blocking::Client::builder()
        .user_agent(concat!("WorshipPlaybackEngine/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(None)
        .build()?;
    let mut done_before = 0u64;
    for f in &def.files {
        let dest = dir.join(f.filename);
        if file_ok(&dir, f) && verify(&dest, &f.checksum).is_ok() {
            done_before += f.size;
            continue;
        }
        let part = dir.join(format!("{}.part", f.filename));
        let mut have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        if have > f.size {
            let _ = std::fs::remove_file(&part);
            have = 0;
        }
        if have < f.size {
            let mut req = client.get(f.url);
            if have > 0 {
                req = req.header("Range", format!("bytes={have}-"));
            }
            let mut resp = req.send().with_context(|| format!("Could not reach {}", f.url))?;
            if !resp.status().is_success() {
                return Err(anyhow!("Server returned {} for {}", resp.status(), f.filename));
            }
            let append = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
            if !append {
                have = 0;
            }
            let mut out = std::fs::OpenOptions::new()
                .create(true)
                .append(append)
                .write(true)
                .truncate(!append)
                .open(&part)?;
            let mut buf = vec![0u8; 256 * 1024];
            let mut last = Instant::now();
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err(anyhow!("cancelled"));
                }
                let n = resp.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n])?;
                have += n as u64;
                if last.elapsed().as_millis() > 200 {
                    progress(done_before + have, format!("Downloading {}", f.filename));
                    last = Instant::now();
                }
            }
            out.flush()?;
        }
        progress(done_before + have, format!("Verifying {}", f.filename));
        if let Err(e) = verify(&part, &f.checksum) {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        std::fs::rename(&part, &dest)?;
        done_before += f.size;
    }
    for (name, content) in &def.extra {
        std::fs::write(dir.join(name), content)?;
    }
    Ok(())
}

// ---------------- Python environment ----------------

static ENV_BUSY: AtomicBool = AtomicBool::new(false);

fn run_uv_python_and_sync(ctx: &Ctx, uv: &Path, emit: &dyn Fn(&str, bool, Option<String>)) -> Result<()> {
    let paths = ctx.paths.read().clone();
    let install_dir = paths.python_install();
    emit("Mengunduh Python 3.12 (uv)…", false, None);
    let mut py = Command::new(uv);
    py.args(["python", "install", "3.12"])
        .env("UV_PYTHON_INSTALL_DIR", &install_dir)
        .env("UV_NO_PROGRESS", "1");
    let st = py.status().context("uv python install")?;
    if !st.success() {
        return Err(anyhow!("uv python install failed"));
    }
    emit("Menginstal paket audio (uv sync)…", false, None);
    let mut child = Command::new(uv)
        .args(["sync", "--frozen", "--no-dev", "--python", "3.12"])
        .current_dir(&ctx.worker_dir)
        .env("UV_PYTHON_INSTALL_DIR", &install_dir)
        .env("UV_PROJECT_ENVIRONMENT", paths.python_env())
        .env("UV_NO_PROGRESS", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stderr_log = String::new();
    if let Some(err) = child.stderr.take() {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            let l = line.trim();
            if !l.is_empty() {
                stderr_log.push_str(l);
                stderr_log.push('\n');
                logs::info(cat::SYSTEM, format!("uv: {l}"));
                emit(l, false, None);
            }
        }
    }
    let st = child.wait()?;
    if !st.success() {
        let msg = if stderr_log.trim().is_empty() {
            "uv sync failed. See Diagnostics → Logs.".into()
        } else {
            crate::compat::humanize_uv_sync_error(&stderr_log)
        };
        return Err(anyhow!("{msg}"));
    }
    Ok(())
}

/// Synchronous engine install (used by setup wizard).
pub fn install_python_env_sync(ctx: Arc<Ctx>) -> Result<()> {
    let uv = find_tool("uv").ok_or_else(|| anyhow!("Bundled uv sidecar not found. Reinstall the app."))?;
    if ENV_BUSY.swap(true, Ordering::SeqCst) {
        return Err(anyhow!("The Python environment is already being installed"));
    }
    let emit = |msg: &str, done: bool, error: Option<String>| {
        let _ = ctx.app.emit(
            "resource:progress",
            serde_json::json!({ "id": "python-env", "progress": { "message": msg, "active": !done, "error": error, "downloaded": 0, "total": 0 } }),
        );
    };
    let result = run_uv_python_and_sync(&ctx, &uv, &emit);
    match result {
        Ok(()) => emit("Python environment ready", true, None),
        Err(e) => {
            emit(&format!("{e:#}"), true, Some(format!("{e:#}")));
            ENV_BUSY.store(false, Ordering::SeqCst);
            return Err(e);
        }
    }
    ENV_BUSY.store(false, Ordering::SeqCst);
    let _ = ctx.app.emit("resource:changed", "python-env");
    Ok(())
}

/// Build the local Python environment from the locked dependency list using bundled `uv`.
pub fn install_python_env(ctx: Arc<Ctx>) -> Result<()> {
    let uv = find_tool("uv").ok_or_else(|| anyhow!("Bundled uv sidecar not found. Reinstall the app."))?;
    if ENV_BUSY.swap(true, Ordering::SeqCst) {
        return Err(anyhow!("The Python environment is already being installed"));
    }
    std::thread::spawn(move || {
        let emit = |msg: &str, done: bool, error: Option<String>| {
            let _ = ctx.app.emit(
                "resource:progress",
                serde_json::json!({ "id": "python-env", "progress": { "message": msg, "active": !done, "error": error, "downloaded": 0, "total": 0 } }),
            );
        };
        let result = run_uv_python_and_sync(&ctx, &uv, &emit);
        match result {
            Ok(()) => emit("Python environment ready", true, None),
            Err(e) => emit(&format!("{e:#}"), true, Some(format!("{e:#}"))),
        }
        ENV_BUSY.store(false, Ordering::SeqCst);
        let _ = ctx.app.emit("resource:changed", "python-env");
    });
    Ok(())
}

pub fn download_with_retries(ctx: Arc<Ctx>, id: &str, attempts: u32) -> Result<()> {
    let mut delay_ms = 2000u64;
    let mut last = None;
    for attempt in 1..=attempts {
        if let Err(e) = download_blocking(ctx.clone(), id) {
            last = Some(e);
            if attempt < attempts {
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                delay_ms = (delay_ms * 2).min(60_000);
            }
        } else {
            return Ok(());
        }
    }
    Err(last.unwrap_or_else(|| anyhow!("download failed")))
}

fn download_blocking(ctx: Arc<Ctx>, id: &str) -> Result<()> {
    let def = catalog().into_iter().find(|d| d.id == id).ok_or_else(|| anyhow!("Unknown resource"))?;
    let flag = Arc::new(AtomicBool::new(false));
    let total: u64 = def.files.iter().map(|f| f.size).sum();
    ctx.resources.set_progress(
        &ctx,
        id,
        Progress {
            total,
            active: true,
            message: "Starting download".into(),
            ..Default::default()
        },
    );
    download_resource(&ctx, &def, &flag, |downloaded, msg| {
        ctx.resources.set_progress(
            &ctx,
            id,
            Progress {
                downloaded,
                total,
                message: msg,
                active: true,
                error: None,
            },
        );
    })?;
    let paths = ctx.paths.read().clone();
    let dir = resource_dir(&paths, &def);
    let _ = ctx.db.set_resource(
        def.id,
        def.kind,
        def.name,
        def.version,
        "installed",
        total,
        Some(&dir.to_string_lossy()),
        Some(&serde_json::json!({ "license": def.license, "source": def.source })),
    );
    ctx.resources.set_progress(
        &ctx,
        id,
        Progress {
            downloaded: total,
            total,
            message: "Installed and verified".into(),
            active: false,
            error: None,
        },
    );
    let _ = ctx.app.emit("resource:changed", id);
    Ok(())
}

pub fn dir_size(p: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else { return 0 };
    rd.filter_map(|e| e.ok())
        .map(|e| {
            let path = e.path();
            if path.is_dir() {
                dir_size(&path)
            } else {
                e.metadata().map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_complete() {
        for d in catalog() {
            assert!(!d.license.is_empty() && !d.source.is_empty() && !d.version.is_empty());
            assert!(d.files.iter().all(|f| f.size > 0 && f.url.starts_with("https://")));
        }
    }

    #[test]
    fn checksum_verification() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.bin");
        std::fs::write(&p, b"hello").unwrap();
        // sha256("hello") = 2cf24dba...
        assert!(verify(&p, &Checksum::Sha256Prefix("2cf24dba")).is_ok());
        assert!(verify(&p, &Checksum::Sha256Prefix("deadbeef")).is_err());
        assert!(verify(
            &p,
            &Checksum::Sha256("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
        )
        .is_ok());
    }
}
