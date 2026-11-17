"""Command-line entry: `python -m wpe_worker <command>`.

    prepare --request req.json   full preparation, JSON-lines events on stdout
    analyze <file>               BPM / key / sections only (debugging)
    engines --request req.json   which separation engines are usable
"""

from __future__ import annotations

import argparse
import json
import sys
import warnings

from ..protocol import PrepareRequest, WorkerError, emit


def main(argv: list[str] | None = None) -> int:
    warnings.filterwarnings("ignore")
    ap = argparse.ArgumentParser(prog="wpe_worker")
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("prepare")
    p.add_argument("--request", required=True)
    a = sub.add_parser("analyze")
    a.add_argument("file")
    a.add_argument("--bpb", type=int, default=4)
    e = sub.add_parser("engines")
    e.add_argument("--request", required=True)
    args = ap.parse_args(argv)

    if args.cmd == "prepare":
        try:
            with open(args.request, encoding="utf-8") as f:
                req = PrepareRequest.parse(json.load(f))
        except WorkerError as err:
            emit("error", code=err.code, message=err.message)
            return 2
        except Exception as err:  # noqa: BLE001
            emit("error", code="bad_request", message=f"Invalid request: {err}")
            return 2
        from ..pipeline import run_and_emit

        return run_and_emit(req)

    if args.cmd == "analyze":
        import soundfile as sf

        from ..analysis.key import detect_key
        from ..analysis.sections import detect_sections
        from ..analysis.tempo import detect_tempo

        y, sr = sf.read(args.file, dtype="float32", always_2d=True)
        y = y.mean(axis=1)
        t = detect_tempo(y, sr, args.bpb)
        k = detect_key(y, sr)
        s = detect_sections(y, sr, t.bpm, t.beat_offset, args.bpb)
        print(json.dumps({"bpm": t.bpm, "confidence": t.confidence, "offset": t.beat_offset, "key": k, "sections": s}, indent=2))
        return 0

    if args.cmd == "engines":
        from ..separation.registry import describe

        with open(args.request, encoding="utf-8") as f:
            req = PrepareRequest.parse(json.load(f))
        print(json.dumps(describe(req.separation), indent=2))
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
