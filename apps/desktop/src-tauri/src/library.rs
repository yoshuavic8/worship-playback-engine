//! Song library: import, song packages on disk, song.json, duplication and backing re-render.
//!
//!   songs/<id>/
//!     source/original.<ext>
//!     stems/{vocals,drums,bass,other}.wav
//!     rendered/{backing,backup}.wav
//!     analysis/waveform.json
//!     song.json

use crate::audio::buffer::{db_to_gain, AudioBuffer};
use crate::audio::decode;
use crate::db::Db;
use crate::models::*;
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};

pub fn song_path(song: &Song, rel: &str) -> PathBuf {
    Path::new(&song.dir).join(rel)
}

pub fn file_exists(song: &Song, rel: &Option<String>) -> bool {
    rel.as_ref().map(|r| song_path(song, r).is_file()).unwrap_or(false)
}

fn guess_title_artist(path: &Path) -> (String, String) {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Untitled").replace('_', " ");
    if let Some((a, t)) = stem.split_once(" - ") {
        (t.trim().to_string(), a.trim().to_string())
    } else {
        (stem.trim().to_string(), String::new())
    }
}

struct Tags {
    title: Option<String>,
    artist: Option<String>,
    bpm: Option<f64>,
    key: Option<String>,
}

fn read_tags(path: &Path) -> Tags {
    use lofty::prelude::*;
    let mut t = Tags { title: None, artist: None, bpm: None, key: None };
    let Ok(probe) = lofty::probe::Probe::open(path) else { return t };
    let Ok(tagged) = probe.read() else { return t };
    let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) else { return t };
    t.title = tag.title().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    t.artist = tag.artist().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    t.bpm = tag
        .get_string(&lofty::tag::ItemKey::Bpm)
        .and_then(|s| s.trim().parse::<f64>().ok())
        .filter(|b| (30.0..=250.0).contains(b));
    t.key = tag
        .get_string(&lofty::tag::ItemKey::InitialKey)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.len() <= 4);
    t
}

fn unique_id(db: &Db, base: &str, songs_dir: &Path) -> Result<String> {
    let mut id = base.to_string();
    let mut n = 2;
    while db.song_id_exists(&id)? || songs_dir.join(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    Ok(id)
}

pub fn import_file(db: &Db, songs_dir: &Path, src: &Path, defaults: &PrepSettings) -> Result<Song> {
    if !src.is_file() {
        return Err(anyhow!("File not found: {}", src.display()));
    }
    if !decode::is_supported_audio(src) {
        return Err(anyhow!(
            "Unsupported file type. Use MP3, WAV, FLAC, M4A or AIFF ({}).",
            src.display()
        ));
    }
    let (buf, info) = decode::decode_file(src).context("Could not read this audio file")?;
    if info.duration < 1.0 {
        return Err(anyhow!("The audio file is too short ({:.1}s)", info.duration));
    }
    let tags = read_tags(src);
    let (gt, ga) = guess_title_artist(src);
    let title = tags.title.unwrap_or(gt);
    let artist = tags.artist.unwrap_or(ga);
    let id = unique_id(db, &slugify(&title), songs_dir)?;
    let dir = songs_dir.join(&id);
    for sub in ["source", "stems", "rendered", "analysis"] {
        std::fs::create_dir_all(dir.join(sub))?;
    }
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("audio").to_ascii_lowercase();
    let rel_source = format!("source/original.{ext}");
    std::fs::copy(src, dir.join(&rel_source)).context("copy source file")?;
    let wf = decode::waveform_peaks(&buf, 50);
    std::fs::write(dir.join("analysis/waveform.json"), serde_json::to_vec(&wf)?)?;

    let now = now_iso();
    let song = Song {
        id: id.clone(),
        title,
        artist,
        bpm: tags.bpm,
        key: tags.key,
        key_pitch_class: None,
        key_mode: None,
        time_signature: "4/4".into(),
        beat_offset: 0.0,
        duration: info.duration,
        sample_rate: info.sample_rate,
        channels: info.channels,
        dir: dir.to_string_lossy().to_string(),
        source_path: rel_source,
        stems: StemPaths::default(),
        rendered: RenderedPaths::default(),
        status: SongStatus::Imported,
        prep: defaults.clone(),
        routing: SongRouting::default(),
        sections: vec![],
        arrangement: vec![],
        count_in_bars: 1,
        cue_lead_beats: 4,
        notes: String::new(),
        last_error: None,
        analysis: None,
        grid: None,
        created_at: now.clone(),
        updated_at: now,
        prepared_at: None,
    };
    db.upsert_song(&song)?;
    write_song_json(&song)?;
    crate::logs::info(crate::logs::cat::SYSTEM, format!("Imported \"{}\" ({:.0}s)", song.title, song.duration));
    Ok(song)
}

/// Portable song package description (`song.json`), rewritten on every save.
pub fn write_song_json(song: &Song) -> Result<()> {
    let j = serde_json::json!({
        "id": song.id,
        "title": song.title,
        "artist": song.artist,
        "bpm": song.bpm,
        "key": song.key,
        "timeSignature": song.time_signature,
        "beatOffset": song.beat_offset,
        "duration": song.duration,
        "sampleRate": song.sample_rate,
        "source": song.source_path,
        "stems": song.stems,
        "rendered": song.rendered,
        "prep": song.prep,
        "sections": song.sections,
        "arrangement": song.arrangement,
        "routing": {
            "foh": { "backing": song.routing.backing_to_foh },
            "monitor": {
                "backing": song.routing.backing_to_monitor,
                "guideVocal": song.routing.guide_vocal_to_monitor,
                "click": song.routing.click_enabled,
                "cue": song.routing.cue_enabled
            }
        },
        "updatedAt": song.updated_at,
    });
    let dir = Path::new(&song.dir);
    if dir.exists() {
        std::fs::write(dir.join("song.json"), serde_json::to_vec_pretty(&j)?)?;
    }
    Ok(())
}

pub fn duplicate_song(db: &Db, songs_dir: &Path, song: &Song) -> Result<Song> {
    let id = unique_id(db, &format!("{}-copy", song.id), songs_dir)?;
    let dir = songs_dir.join(&id);
    copy_dir(Path::new(&song.dir), &dir)?;
    let mut s = song.clone();
    s.id = id.clone();
    s.title = format!("{} (Copy)", song.title);
    s.dir = dir.to_string_lossy().to_string();
    s.created_at = now_iso();
    s.updated_at = now_iso();
    for sec in s.sections.iter_mut() {
        let old = sec.id.clone();
        sec.id = new_id();
        for a in s.arrangement.iter_mut() {
            if *a == old {
                *a = sec.id.clone();
            }
        }
    }
    db.upsert_song(&s)?;
    write_song_json(&s)?;
    Ok(s)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let p = e.path();
        let t = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &t)?;
        } else {
            std::fs::copy(&p, &t)?;
        }
    }
    Ok(())
}

pub fn delete_song_files(song: &Song, songs_dir: &Path) -> Result<()> {
    let dir = Path::new(&song.dir);
    // Only ever delete inside the library songs directory.
    if dir.starts_with(songs_dir) && dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

pub fn stem_gain(mode: StemMode, db: f32) -> f32 {
    match mode {
        StemMode::Remove | StemMode::Optional => 0.0,
        StemMode::Keep => db_to_gain(db),
    }
}

/// Mix the backing from existing stems: backing = other + adjusted bass + optional drums.
/// Fast (no AI) — used when the MD changes the preset after preparation.
pub fn render_backing_from_stems(song: &Song) -> Result<(PathBuf, PathBuf)> {
    let load = |rel: &Option<String>| -> Result<AudioBuffer> {
        let rel = rel.as_ref().ok_or_else(|| anyhow!("missing stem"))?;
        let (b, _) = decode::decode_file(&song_path(song, rel))?;
        decode::resample(&b, 48000)
    };
    let other = load(&song.stems.other)?;
    let bass = load(&song.stems.bass)?;
    let drums = load(&song.stems.drums)?;
    let p = &song.prep;
    let gains = [
        stem_gain(StemMode::Keep, p.other_db),
        stem_gain(p.bass, p.bass_db),
        stem_gain(p.drums, 0.0),
    ];
    let len = other.data.len().max(bass.data.len()).max(drums.data.len());
    let mut backing = vec![0f32; len];
    let mut backup = vec![0f32; len];
    for (i, (b, k)) in backing.iter_mut().zip(backup.iter_mut()).enumerate() {
        let o = other.data.get(i).copied().unwrap_or(0.0);
        let ba = bass.data.get(i).copied().unwrap_or(0.0);
        let d = drums.data.get(i).copied().unwrap_or(0.0);
        *b = o * gains[0] + ba * gains[1] + d * gains[2];
        *k = o + ba + d;
    }
    let dir = Path::new(&song.dir).join("rendered");
    std::fs::create_dir_all(&dir)?;
    let bp = dir.join("backing.wav");
    let kp = dir.join("backup.wav");
    decode::write_wav_f32(&bp, &AudioBuffer::new(backing, 48000))?;
    decode::write_wav_f32(&kp, &AudioBuffer::new(backup, 48000))?;
    Ok((bp, kp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_artist_from_filename() {
        assert_eq!(
            guess_title_artist(Path::new("/x/Bethel Music - Goodness of God.mp3")),
            ("Goodness of God".to_string(), "Bethel Music".to_string())
        );
        assert_eq!(guess_title_artist(Path::new("/x/build_my_life.wav")).0, "build my life");
    }

    #[test]
    fn import_rejects_unsupported_and_accepts_wav() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let songs = tmp.path().join("songs");
        std::fs::create_dir_all(&songs).unwrap();
        let txt = tmp.path().join("notes.txt");
        std::fs::write(&txt, "hi").unwrap();
        assert!(import_file(&db, &songs, &txt, &PrepSettings::default()).is_err());

        let wav = tmp.path().join("Test Artist - Test Song.wav");
        let mono: Vec<f32> = (0..96000).map(|i| ((i as f32) * 0.05).sin() * 0.3).collect();
        decode::write_wav_f32(&wav, &AudioBuffer::from_mono(&mono, 48000)).unwrap();
        let s = import_file(&db, &songs, &wav, &PrepSettings::default()).unwrap();
        assert_eq!(s.title, "Test Song");
        assert_eq!(s.artist, "Test Artist");
        assert!((s.duration - 2.0).abs() < 0.01);
        assert!(Path::new(&s.dir).join("song.json").exists());
        assert!(Path::new(&s.dir).join("analysis/waveform.json").exists());
        // second import gets a unique id
        let s2 = import_file(&db, &songs, &wav, &PrepSettings::default()).unwrap();
        assert_ne!(s.id, s2.id);
    }
}
