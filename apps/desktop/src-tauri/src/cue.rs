//! Cue engine. Spoken cues are rendered once with macOS system TTS (`say`) into local WAV assets,
//! so live playback never depends on TTS (or the network) at performance time.

use crate::models::slugify;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Render (or reuse) a cue clip. Returns the WAV path.
pub fn ensure_cue(cue_root: &Path, voice: Option<&str>, label: &str) -> Result<PathBuf> {
    let text = cue_text(label);
    if text.is_empty() {
        return Err(anyhow!("empty cue label"));
    }
    let path = cue_path(cue_root, voice, &text);
    if path.exists() && std::fs::metadata(&path).map(|m| m.len() > 1024).unwrap_or(false) {
        return Ok(path);
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    let tmp = path.with_extension("tmp.wav");
    let mut cmd = Command::new("/usr/bin/say");
    if let Some(v) = voice {
        cmd.args(["-v", v]);
    }
    cmd.arg("-o").arg(&tmp).args(["--file-format=WAVE", "--data-format=LEF32@48000", "-r", "190"]).arg(&text);
    let out = cmd.output().map_err(|e| anyhow!("macOS speech synthesis unavailable: {e}"))?;
    if !out.status.success() {
        return Err(anyhow!("say failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    std::fs::rename(&tmp, &path)?;
    Ok(path)
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
}
