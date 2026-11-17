import json, sys, numpy as np
from repro import *
from grid_fit import fit_grid
cases = [
 ("A control", dict(bpm=96.0, layout=STD)),
 ("B 2.3s lead-in", dict(bpm=96.0, layout=STD, lead_silence=2.3)),
 ("C lead-in + pickup", dict(bpm=96.0, layout=STD, lead_silence=2.3, pickup=True)),
 ("D 8-bar soft intro 72", dict(bpm=72.0, layout=LONG_SOFT_INTRO, lead_silence=1.5)),
 ("E 2-bar intro", dict(bpm=96.0, layout=ODD_INTRO, lead_silence=0.8)),
 ("F 71.8", dict(bpm=71.8, layout=STD, lead_silence=0.5)),
 ("H 72.15", dict(bpm=72.15, layout=STD, lead_silence=0.5)),
 ("I breakdown", dict(bpm=96.0, layout=STD[:5]+[("Bridge",8,0.3,False)]+STD[6:], lead_silence=0.5)),
 ("G 124.6 noise", dict(bpm=124.6, layout=STD, lead_silence=1.0, noise_db=-45)),
]
rows = []
for name, kw in cases:
    y, tr = make_song(**kw)
    g = fit_grid(y, SR)
    beat = 60 / tr["bpm"]
    last = tr["beats"][-1]; k = round((last - tr["beat1"]) / beat)
    pred = g["beat1"] + k * 60 / g["bpm"]
    row = dict(case=name, true_bpm=tr["bpm"], bpm=g["bpm"], true_beat1=round(tr["beat1"],3), beat1=g["beat1"],
               beat1_err_ms=round((g["beat1"]-tr["beat1"])*1000), end_err_ms=round((pred-last)*1000),
               resid_ms=g["resid_ms"], db_conf=g["downbeat_conf"])
    rows.append(row); print(json.dumps(row)); sys.stdout.flush()
json.dump(rows, open("compare_results.json","w"), indent=1)
