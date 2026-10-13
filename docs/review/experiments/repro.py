"""Reproduce WPE analysis failures on synthetic worship-like songs with known ground truth."""
import sys, json
import numpy as np
sys.path.insert(0, "/workspace/wpe/Worship Playback Engine/audio-worker/src")
from wpe_worker import testaudio as ta
from wpe_worker.analysis.tempo import detect_tempo
from wpe_worker.analysis.sections import detect_sections
from wpe_worker.audio.io import resample

SR = 22050

def make_song(bpm, layout, lead_silence=0.0, pickup=False, seed=7, noise_db=None):
    """layout: list of (label, bars, energy, has_drums). Returns mono y@22050, truth dict."""
    rng = np.random.default_rng(seed)
    sr = SR
    beat = 60.0 / bpm; bar = 4 * beat
    prog = {"Intro": ["A","E"], "Verse": ["A","D","A","E"], "Chorus": ["D","A","E","F#"],
            "Bridge": ["B","D","E","E"], "Outro": ["A","A"]}
    total = sum(b for _, b, _, _ in layout)
    t0song = lead_silence
    n = int((t0song + total * bar) * sr) + sr
    y = np.zeros(n, np.float32)
    kick = ta._kick(sr)
    secs = []; bar_i = 0
    if pickup:  # one-beat pickup: snare fill + bass note on beat 4 before bar 1
        ps = int((t0song - beat) * sr)
        sn = ta._snare(sr, rng); y[ps:ps+len(sn)] += sn * 0.4
    for label, nb, energy, drums in layout:
        secs.append((label, t0song + bar_i * bar))
        chords = prog[label.split()[0]]
        for b in range(nb):
            t0 = t0song + (bar_i + b) * bar
            r = ta.NOTE[chords[b % len(chords)]]
            s0 = int(t0 * sr); seg = int(bar * sr); t = np.arange(seg) / sr
            env = np.minimum(1, t * 20) * np.minimum(1, (bar - t) * 20)
            chord = sum(np.sin(2*np.pi*ta._hz(r+iv-12)*t) for iv in (0,4,7))
            y[s0:s0+seg] += (chord * 0.12 * (0.5+energy) * env).astype(np.float32)[:n-s0]
            if drums:
                for k in range(4):
                    bs = int((t0 + k*beat) * sr); bt = np.arange(int(beat*0.9*sr)) / sr
                    note = np.sin(2*np.pi*ta._hz(r-36)*bt) * np.exp(-bt*3) * 0.35 * energy
                    y[bs:bs+len(bt)] += note[:max(0, n-bs)]
                    if k in (0, 2):
                        y[bs:bs+len(kick)] += (kick*0.9*energy)[:max(0, n-bs)]
                    else:
                        sn = ta._snare(sr, rng); y[bs:bs+len(sn)] += (sn*0.5*energy)[:max(0, n-bs)]
        bar_i += nb
    if noise_db is not None:
        y += (rng.standard_normal(n) * 10**(noise_db/20)).astype(np.float32)
    y /= max(1e-9, np.abs(y).max()) / 0.9
    beats = t0song + np.arange(total * 4) * beat
    return y, {"bpm": bpm, "beat1": t0song, "beats": beats, "sections": secs, "dur": n / sr}

STD = [("Intro",4,0.35,False),("Verse 1",8,0.6,True),("Chorus",8,1.0,True),("Verse 2",8,0.6,True),
       ("Chorus",8,1.0,True),("Bridge",8,0.8,True),("Chorus",8,1.0,True),("Outro",4,0.3,False)]
LONG_SOFT_INTRO = [("Intro",8,0.25,False)] + STD[1:]
ODD_INTRO = [("Intro",2,0.35,False)] + STD[1:]   # 2-bar intro: phrase grid no longer aligned to 4 bars

def evaluate(name, y, truth):
    r = detect_tempo(y, SR, 4)
    bpm_t = truth["bpm"]; beat = 60 / bpm_t; bar = 4 * beat
    out = {"case": name, "true_bpm": bpm_t, "bpm": r.bpm, "alts": r.alternatives,
           "true_beat1": round(truth["beat1"], 3), "beat1": r.beat_offset,
           "musical_start": r.musical_start, "audio_start": r.audio_start,
           "mode": r.tempo_mode, "stability": r.stability, "regions": len(r.tempo_map),
           "conf": r.confidence, "db_conf": r.downbeat_confidence,
           "first_tracked_beat": round(r.beats[0], 3) if r.beats else None}
    if r.bpm:
        e = r.beat_offset - truth["beat1"]
        out["beat1_err_beats"] = round(e / beat, 2)
        # fixed-formula drift at last true beat, as player.rs does it (bpm rounded to 0.1 in prep.rs:439)
        bpm_rust = round(r.bpm * 10) / 10
        last = truth["beats"][-1]
        k = round((last - truth["beat1"]) / beat)
        pred = r.beat_offset + k * 60 / bpm_rust
        d = (pred - last) % beat
        if d > beat / 2: d -= beat
        out["click_phase_err_at_end_ms"] = round(d * 1000)   # wrapped to +-half beat
        out["beat1_bar_aligned"] = abs(e / bar - round(e / bar)) < 0.05
        tb = np.asarray(r.beats); jit = [np.min(np.abs(truth["beats"] - t)) for t in tb]
        out["raw_beats_max_err_ms"] = round(float(np.max(jit)) * 1000)
        out["raw_beats_count_vs_true"] = f"{len(tb)}/{len(truth['beats'])}"
        secs = detect_sections(y, SR, r.bpm, r.beat_offset, 4)
        starts = [s["start"] for s in secs]
        errs = [min(abs(t - s) for s in starts) for _, t in truth["sections"][1:]]
        out["sections"] = [(s["label"], s["start"]) for s in secs]
        out["section_hits_within_0.1s"] = f"{sum(e < 0.1 for e in errs)}/{len(errs)}"
        out["section_hits_within_1bar"] = f"{sum(e <= bar + 0.01 for e in errs)}/{len(errs)}"
    return out

if __name__ == "__main__":
    cases = [
        ("A control 96bpm, starts at 0", dict(bpm=96.0, layout=STD)),
        ("B 2.3s silence lead-in", dict(bpm=96.0, layout=STD, lead_silence=2.3)),
        ("C 2.3s silence + 1-beat pickup", dict(bpm=96.0, layout=STD, lead_silence=2.3, pickup=True)),
        ("D 8-bar soft pad intro (no drums) + 1.5s lead", dict(bpm=72.0, layout=LONG_SOFT_INTRO, lead_silence=1.5)),
        ("E 2-bar intro (phrase misaligned)", dict(bpm=96.0, layout=ODD_INTRO, lead_silence=0.8)),
        ("F non-integer 71.8bpm click-recorded", dict(bpm=71.8, layout=STD, lead_silence=0.5)),
        ("H 72.15bpm (gets rounded to 72)", dict(bpm=72.15, layout=STD, lead_silence=0.5)),
        ("I breakdown: bridge w/o drums", dict(bpm=96.0, layout=STD[:5]+[("Bridge",8,0.3,False)]+STD[6:], lead_silence=0.5)),
        ("G 124.6bpm + noise floor", dict(bpm=124.6, layout=STD, lead_silence=1.0, noise_db=-45)),
    ]
    res = []
    for name, kw in cases:
        y, truth = make_song(**kw)
        o = evaluate(name, y, truth); res.append(o)
        print(json.dumps(o, default=float)); sys.stdout.flush()
    json.dump(res, open("/workspace/wpe/experiments/repro_results.json", "w"), indent=1, default=float)
