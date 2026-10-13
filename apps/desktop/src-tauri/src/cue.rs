//! Cue engine. Spoken cues are rendered once with macOS system TTS (`say`) into local WAV assets,
//! so live playback never depends on TTS (or the network) at performance time.

use crate::models::slugify;
use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// One render at a time per destination file. Save and load both call this,
/// and they used to share one `<slug>.tmp.wav`, so the second rename failed
/// with "No such file or directory".
fn cue_locks() -> &'static Mutex<HashSet<PathBuf>> {
    static LOCKS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashSet::new()))
}

struct CueGuard(PathBuf);

impl Drop for CueGuard {
    fn drop(&mut self) {
        cue_locks().lock().remove(&self.0);
    }
}

fn lock_cue(path: &Path) -> CueGuard {
    loop {
        let mut held = cue_locks().lock();
        if !held.contains(path) {
            held.insert(path.to_path_buf());
            return CueGuard(path.to_path_buf());
        }
        drop(held);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    pub name: String,
    pub locale: String,
}

pub fn list_voices() -> Vec<Voice> {
    let Ok(out) = Command::new("/usr/bin/say").args(["-v", "?"]).output() else { return vec![] };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut voices = vec![];
    for line in text.lines() {
        let left = line.split('#').next().unwrap_or("").trim();
        let Some(idx) = left.rfind(char::is_whitespace) else { continue };
        let (name, locale) = left.split_at(idx);
        let locale = locale.trim();
        if !locale.contains('_') {
            continue;
        }
        let name = name.trim().to_string();
        if !voices.iter().any(|v: &Voice| v.name == name) {
            voices.push(Voice { name, locale: locale.into() });
        }
    }
    voices
}

/// The phrase spoken for a section label, e.g. "Pre-Chorus" → "Pre Chorus".
pub fn cue_text(label: &str) -> String {
    label.replace(['-', '_'], " ").split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn cue_path(cue_root: &Path, voice: Option<&str>, text: &str) -> PathBuf {
    let vdir = voice.map(slugify).unwrap_or_else(|| "system".into());
    cue_root.join(vdir).join(format!("{}.wav", slugify(text)))
}

fn audio_ok(path: &Path) -> bool {
    std::fs::metadata(path).map(|m| m.len() > 1024).unwrap_or(false)
}

fn unique_tmp(path: &Path) -> PathBuf {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let tid = format!("{:?}", std::thread::current().id());
    let tid: String = tid.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("cue");
    path.with_file_name(format!("{stem}.{}.{tid}.{nanos}.tmp.wav", std::process::id()))
}

fn run_say(voice: Option<&str>, tmp: &Path, text: &str) -> Result<()> {
    let mut cmd = Command::new("/usr/bin/say");
    if let Some(v) = voice.filter(|v| !v.is_empty()) {
        cmd.args(["-v", v]);
    }
    cmd.arg("-o")
        .arg(tmp)
        .args(["--file-format=WAVE", "--data-format=LEF32@48000", "-r", "190"])
        .arg(text);
    let out = cmd.output().map_err(|e| anyhow!("macOS speech synthesis unavailable: {e}"))?;
    if !out.status.success() {
        return Err(anyhow!(
            "say failed writing {}: {}",
            tmp.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

/// Render (or reuse) a cue clip. Returns the WAV path.
pub fn ensure_cue(cue_root: &Path, voice: Option<&str>, label: &str) -> Result<PathBuf> {
    let text = cue_text(label);
    if text.is_empty() {
        return Err(anyhow!("empty cue label"));
    }
    let path = cue_path(cue_root, voice, &text);
    let _guard = lock_cue(&path);
    if audio_ok(&path) {
        return Ok(path);
    }
    let parent = path.parent().context(format!("cue path has no directory: {}", path.display()))?;
    std::fs::create_dir_all(parent).with_context(|| format!("create cue directory {}", parent.display()))?;
    let tmp = unique_tmp(&path);
    let voiced = voice.filter(|v| !v.is_empty());
    let first = run_say(voiced, &tmp, &text);
    if !audio_ok(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        if voiced.is_some() {
            run_say(None, &tmp, &text).with_context(|| {
                format!("system voice fallback after {:?} failed for '{text}'", voiced)
            })?;
        } else {
            first.with_context(|| format!("say produced nothing for '{text}' at {}", tmp.display()))?;
        }
    }
    if !audio_ok(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(anyhow!(
            "say produced no audio for voice '{}' (text '{text}') at {}",
            voiced.unwrap_or("system"),
            tmp.display()
        ));
    }
    match std::fs::rename(&tmp, &path) {
        Ok(()) => Ok(path),
        Err(err) => {
            let _ = std::fs::remove_file(&tmp);
            if audio_ok(&path) {
                Ok(path)
            } else {
                Err(err).with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))
            }
        }
    }
}

/// Built-in spoken test phrase for Devices → Test Cue.
pub fn ensure_test_cue(cue_root: &Path, voice: Option<&str>) -> Result<PathBuf> {
    ensure_cue(cue_root, voice, "Monitor cue test")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cue_text_normalizes() {
        assert_eq!(cue_text("Pre-Chorus"), "Pre Chorus");
        assert_eq!(cue_text("  Verse   1 "), "Verse 1");
    }

    #[test]
    fn cue_paths_are_stable() {
        let p = cue_path(Path::new("/x"), Some("Samantha"), "Verse 1");
        assert_eq!(p, PathBuf::from("/x/samantha/verse-1.wav"));
    }

    /// Two callers (save in the background, load on the main path) must not
    /// trip over the same temporary file.
    #[cfg(target_os = "macos")]
    #[test]
    fn parallel_renders_of_the_same_label_all_succeed() {
        let dir = std::env::temp_dir().join(format!("wpe-cue-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut handles = Vec::new();
        for _ in 0..8 {
            let root = dir.clone();
            handles.push(std::thread::spawn(move || ensure_cue(&root, None, "Chorus")));
        }
        for handle in handles {
            let path = handle.join().expect("cue thread").expect("cue render");
            let len = std::fs::metadata(&path).expect("cue file").len();
            assert!(len > 1024, "cue wav too small: {len}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
