import { useMemo, useRef, type ReactNode } from "react";

import coking from "../data/coking.json";
import rul from "../data/rul.json";
import { ASSETS } from "../film/sequences";
import { Scene } from "../scenes/Scene";
import { DetectChart, DiagnoseChart, PredictChart } from "./charts";

const lead = (coking.alarms.cusum_s! - coking.onset_s).toFixed(1);
const last = coking.posterior.p[coking.posterior.p.length - 1];
const pct = (name: string) => Math.round((last[coking.posterior.names.indexOf(name)] ?? 0) * 100);
const fuelHealth = coking.health.final[coking.health.names.indexOf("FUEL / INJECTION")];

type Card = {
  key: string;
  title: ReactNode;
  bullets: string[];
  note?: string;
  visual: (bind: Binder) => ReactNode;
};
type Binder = (f: (lp: number) => void) => void;

const CARDS: Card[] = [
  {
    key: "Detect",
    title: (
      <>
        Caught before any limit
        <br />
        <span className="accent">{lead} s after onset</span>
      </>
    ),
    bullets: [
      "CUSUM on every residual, slack 0.5 σ, decision interval 5",
      "Mahalanobis distance across all 22 channels",
      "60 s baseline first, so a healthy engine stays quiet",
    ],
    note: `Offline mission, seed 0x5EED. Live bus run: T+100.3 s.`,
    visual: () => <DetectChart />,
  },
  {
    key: "Diagnose",
    title: (
      <>
        Names the fault
        <br />
        <span className="accent">and the cylinder</span>
      </>
    ),
    bullets: [
      "9 fault hypotheses × 22 channels",
      "Generated from the model, not learned",
      `Coking vs misfire: ${pct("INJECTOR 3 COKING")}% to ${pct("CYLINDER 3 MISFIRE")}%`,
    ],
    visual: (bind) => <DiagnoseChart bind={bind} />,
  },
  {
    key: "Predict",
    title: (
      <>
        Hours left
        <br />
        <span className="accent">with error bars</span>
      </>
    ),
    bullets: [
      `7 health indices. Fuel / injection at ${fuelHealth}, the rest near 100`,
      "Each decline projected to its failure threshold",
      `p10 ${rul.p10_h} h · p90 ${rul.p90_h} h, from the filter covariance`,
    ],
    note: "One-hour mission, injector 3 coking over three hours",
    visual: () => <PredictChart />,
  },
  {
    key: "Replay",
    title: (
      <>
        Replay any flight
        <br />
        <span className="accent">on the same screens</span>
      </>
    ),
    bullets: [
      "Every mission recorded to Parquet, frame for frame",
      "Altitude, hot days, power changes, simulated on the same model",
      "Above 20,000 ft: extrapolated, and marked",
    ],
    visual: () => (
      <Footage src={`${ASSETS}/media/S9.mp4`} tag="32,000 ft · extrapolated above 20,000" />
    ),
  },
  {
    key: "Ground station",
    title: (
      <>
        Read at a glance
        <br />
        <span className="accent">from across the room</span>
      </>
    ),
    bullets: [
      "Solid is measured, dashed is the twin",
      "Orange: look. Red: act. Healthy stays quiet",
      "Rust from the CAN bus to the twin",
    ],
    // MOCK: real GCS screenshots composited onto the monitor wall replace the empty screens.
    visual: () => <Footage src={`${ASSETS}/media/S10.mp4`} tag="ground control station" />,
  },
];

function Footage({ src, tag }: { src: string; tag: string }) {
  return (
    <div className="footage">
      <video src={src} autoPlay muted loop playsInline preload="none" />
      <span className="chip">{tag}</span>
    </div>
  );
}

const N = CARDS.length;

/** Armory's card grammar: a pinned frame, copy left, a live panel right, one card per scroll window. */
export function Cards() {
  const root = useRef<HTMLDivElement>(null);
  // Per-card updaters for the panels that step through data rather than clip it.
  // A plain array: written from child effects, read from the scroll callback.
  const binders = useMemo<((lp: number) => void)[]>(() => [], []);
  const onP = (p: number) => {
    const k = Math.min(N - 1, Math.floor(p * N));
    const lp = p * N - k;
    root.current?.style.setProperty("--lp", lp.toFixed(4));
    root.current?.style.setProperty("--k", String(k));
    binders[k]?.(lp);
  };
  return (
    <Scene vh={N * 150} map={null} opaque stage="cards-stage" onP={onP} id="proof">
      <div className="cards" ref={root}>
        <div className="card-left">
          {CARDS.map((c, k) => (
            <div
              key={c.key}
              className="card-copy"
              data-at={`${k / N + 0.005},${k === N - 1 ? 1.01 : (k + 1) / N - 0.005}`}
            >
              <div className="card-head">
                <span className="card-kicker">
                  {String(k + 1).padStart(2, "0")} / {String(N).padStart(2, "0")} · {c.key}
                </span>
                <h2 className="card-title" data-words>
                  {c.title}
                </h2>
              </div>
              <div className="card-foot">
                <ul>
                  {c.bullets.map((b) => (
                    <li key={b}>{b}</li>
                  ))}
                </ul>
                {c.note && <p className="card-note">{c.note}</p>}
              </div>
            </div>
          ))}
          <div className="card-progress">
            <i />
          </div>
        </div>
        <div className="card-right">
          {CARDS.map((c, k) => (
            <div
              key={c.key}
              className="card-visual"
              data-at={`${k / N + 0.005},${k === N - 1 ? 1.01 : (k + 1) / N - 0.005}`}
            >
              {c.visual((f) => void (binders[k] = f))}
            </div>
          ))}
        </div>
      </div>
    </Scene>
  );
}
