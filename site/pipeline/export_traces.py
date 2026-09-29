"""Mission recordings to the JSON the site's cards animate.

Offline only, like everything else in Python here. Inputs are Parquet files written by
`just mission` (the shipping pipeline on mission time), so every trace on the site is
the twin's own output, and every annotation (alarm times, posterior, remaining life) is
computed here from those traces rather than typed into the page.

    uv run --no-project --with pyarrow python site/pipeline/export_traces.py

Recordings (regenerate with):
    just mission --out site/gen/data/coking.parquet --hours 0.1 --fault-cylinder 3 --fault-ramp 120
    just mission --out site/gen/data/coking_slow.parquet --hours 1 --fault-cylinder 3 \
        --fault-onset 300 --fault-ramp 10800
"""

import json
import math
from pathlib import Path

import pyarrow.parquet as pq

ROOT = Path(__file__).resolve().parents[1]
DATA = ROOT / "gen" / "data"
OUT = ROOT / "src" / "data"

CHANNELS = [
    "rpm", "map", "mat", "maf", "turbo", "torque", "fuel", "oil_p", "oil_t", "coolant",
    "egt_1", "egt_2", "egt_3", "egt_4", "cht_1", "cht_2", "cht_3", "cht_4",
    "lambda_1", "lambda_2", "lambda_3", "lambda_4",
]
# twin-core/src/signature.rs CATALOGUE order, which posterior_<k> follows.
HYPOTHESES = [
    "NOMINAL", "INJECTOR 3 COKING", "CYLINDER 3 MISFIRE", "EGT 3 SENSOR DRIFT",
    "RADIATOR FOULING", "COMPRESSOR EROSION", "TURBINE EROSION", "OIL SUPPLY LOSS",
    "INTAKE RESTRICTION",
]
HEALTH = ["combustion", "thermal", "lubrication", "air_path", "fuel_/_injection", "electrical", "mechanical"]


def clean(v, nd=3):
    """None for NaN or missing, so the page never receives a NaN it could draw."""
    if v is None or (isinstance(v, float) and not math.isfinite(v)):
        return None
    return round(float(v), nd)


def first(t, pred):
    """Index of the first frame satisfying pred, or None."""
    return next((i for i in range(len(t)) if pred(i)), None)


def coking():
    d = pq.read_table(DATA / "coking.parquet").to_pydict()
    t = d["t_s"]
    cal_end = t[first(t, lambda i: not d["calibrating"][i])]

    # Alarms after calibration only: the 60 s per-channel baseline has to be averaged
    # before a CUSUM means anything, and the detector ignores alarms inside it.
    ci = first(t, lambda i: t[i] > cal_end and d["cusum"][i] > d["cusum_limit"][i])
    cusum_at = round(t[ci], 2) if ci is not None else None
    cusum_ch = d["cusum_channel"][ci] if ci is not None else None
    run, maha_at = 0, None
    for i in range(len(t)):
        if t[i] <= cal_end:
            continue
        run = run + 1 if d["distance"][i] > d["distance_limit"][i] else 0
        if run == 4:  # four consecutive frames, as detect.rs requires
            maha_at = round(t[i], 2)
            break
    redline = any(v is not None and math.isfinite(v) for v in d["redline_since_s"])

    fast = range(0, len(t), 5)  # 20 Hz to 4 Hz for the traces
    slow = range(0, len(t), 20)  # 1 Hz for the matrix and posterior
    return {
        "source": "just mission --hours 0.1 --fault-cylinder 3 --fault-ramp 120 (seed 0x5EED, cruise)",
        "onset_s": 90.0,
        "ramp_s": 120.0,
        "final_scale": 0.84,
        "calibrated_s": round(cal_end, 2),
        "alarms": {"cusum_s": cusum_at, "cusum_channel": cusum_ch, "mahalanobis_s": maha_at, "redline": redline},
        "trace": {
            "t": [clean(t[i], 2) for i in fast],
            "cusum": [clean(d["cusum"][i], 2) for i in fast],
            "cusum_limit": clean(d["cusum_limit"][len(t) // 2]),
            "distance": [clean(d["distance"][i], 2) for i in fast],
            "distance_limit": clean(d["distance_limit"][len(t) // 2]),
            "egt_3": [clean(d["egt_3"][i], 1) for i in fast],
            "egt_3_hat": [clean(d["egt_3_hat"][i], 1) for i in fast],
            "cht_3": [clean(d["cht_3"][i], 1) for i in fast],
            "cht_3_hat": [clean(d["cht_3_hat"][i], 1) for i in fast],
        },
        "matrix": {
            "channels": [c.upper().replace("_", " ") for c in CHANNELS],
            "t": [clean(t[i], 1) for i in slow],
            "resn": [[clean(d[f"{c}_resn"][i], 2) for c in CHANNELS] for i in slow],
        },
        "posterior": {
            "names": HYPOTHESES,
            "t": [clean(t[i], 1) for i in slow],
            "p": [[clean(d[f"posterior_{k}"][i], 3) for k in range(len(HYPOTHESES))] for i in slow],
        },
        "health": {
            "names": [h.replace("_/_", " / ").replace("_", " ").upper() for h in HEALTH],
            "final": [clean(d[f"health_{h}"][-1], 1) for h in HEALTH],
        },
    }


def rul():
    d = pq.read_table(DATA / "coking_slow.parquet").to_pydict()
    t = d["t_s"]
    step = range(0, len(t), 200)  # 20 Hz to 0.1 Hz; a slow decline needs no more
    key = "injector_3_cd"
    return {
        "source": "just mission --hours 1 --fault-cylinder 3 --fault-onset 300 --fault-ramp 10800 (seed 0x5EED)",
        "parameter": "INJECTOR 3 DISCHARGE COEFFICIENT",
        # Mirrored from twin-core/src/health.rs (INJECTOR_CD_NOMINAL and the injector
        # descriptor's failure threshold). Re-check there if either changes.
        "nominal": 0.966,
        "failure": 0.62,
        "t_h": [clean(t[i] / 3600, 4) for i in step],
        "theta": [clean(d[f"theta_{key}"][i], 4) for i in step],
        "sigma": [clean(d[f"theta_{key}_sigma"][i], 4) for i in step],
        "rul_h": clean(d[f"rul_{key}_h"][-1], 2),
        "p10_h": clean(d[f"rul_{key}_p10_h"][-1], 2),
        "p90_h": clean(d[f"rul_{key}_p90_h"][-1], 2),
    }


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    for name, fn in (("coking", coking), ("rul", rul)):
        path = OUT / f"{name}.json"
        path.write_text(json.dumps(fn(), separators=(",", ":")))
        print(f"{path.relative_to(ROOT)} {path.stat().st_size // 1024} KB")
