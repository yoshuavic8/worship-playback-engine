import json, sys, numpy as np, soundfile as sf
from repro import *
from beat_this.inference import File2Beats
f2b = File2Beats(checkpoint_path="final0", device="cpu", dbn=False)
for name, kw in [("B 2.3s lead-in", dict(bpm=96.0, layout=STD, lead_silence=2.3)),
                 ("C lead-in + pickup", dict(bpm=96.0, layout=STD, lead_silence=2.3, pickup=True)),
                 ("D 8-bar soft intro 72", dict(bpm=72.0, layout=LONG_SOFT_INTRO, lead_silence=1.5)),
                 ("H 72.15", dict(bpm=72.15, layout=STD, lead_silence=0.5))]:
    y, tr = make_song(**kw)
    sf.write("/tmp/case.wav", y, SR)
    beats, downs = f2b("/tmp/case.wav")
    ib = np.median(np.diff(beats))
    print(json.dumps(dict(case=name, true_bpm=tr["bpm"], bpm_median=round(60/ib,2), n_beats=len(beats), n_true=len(tr["beats"]),
        first_beat=round(float(beats[0]),3), first_downbeat=round(float(downs[0]),3), true_beat1=tr["beat1"],
        beat_max_err_ms=round(float(max(np.min(np.abs(tr["beats"]-b)) for b in beats))*1000))))
    sys.stdout.flush()
