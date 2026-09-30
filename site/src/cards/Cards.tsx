import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";

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
  /** Scroll windows this card holds, default 1. */
  span?: number;
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
    // Two windows: the walk up to the wall, then the screens themselves.
    span: 2,
    visual: (bind) => <Screens bind={bind} />,
  },
];

// Captured from the running stack (dragonfly-sim with injector 3 coking, core,
// the GCS) at T+7 min, not drawn: the one card about the product shows the product.
const SCREENS = [
  { src: "gcs-1.webp", tag: "OPS · only the twin fired" },
  { src: "gcs-2.webp", tag: "ANALYSIS · residual matrix" },
  { src: "gcs-3.webp", tag: "TWIN · measured vs physics" },
];

const clamp = (x: number) => Math.max(0, Math.min(1, x));

/** A push-in on the ops room, then the ground station's own screens, all on the card's scroll. */
function Screens({ bind }: { bind: Binder }) {
  const [k, setK] = useState(-1);
  const root = useRef<HTMLDivElement>(null);
  const video = useRef<HTMLVideoElement>(null);
  // S10-push.mp4 is all-intra (every frame a keyframe) so seeking on scroll is
  // instant; the one-keyframe web copy of S10 stalls on every seek.
  // Push-in to 40%, the capture lights up on the wall 34 to 44%, screens stepped
  // through by 90%. The room stays behind them: faded out, it washes to grey on
  // the light panel and the screens lose their place.
  const step = (lp: number) => {
    const v = video.current;
    if (v && v.duration) {
      const t = clamp(lp / 0.4) * (v.duration - 0.05);
      if (Math.abs(v.currentTime - t) > 0.02) v.currentTime = t;
    }
    const fade = clamp((lp - 0.34) / 0.1);
    root.current?.style.setProperty("--in", fade.toFixed(3));
    setK(
      fade < 0.5
        ? -1
        : Math.min(SCREENS.length - 1, Math.floor(((lp - 0.4) / 0.5) * SCREENS.length)),
    );
  };
  useEffect(() => bind(step), [bind]);
  return (
    <div className="screens" ref={root}>
      <video ref={video} src={`${ASSETS}/media/S10-push.mp4`} muted playsInline preload="auto" />
      {SCREENS.map((s, i) => (
        <img
          key={s.src}
          src={`${ASSETS}/media/${s.src}`}
          alt={`DRAGONFLY ground station, ${s.tag}`}
          className={i === Math.max(0, k) ? "is-on" : ""}
          loading="lazy"
          decoding="async"
        />
      ))}
      <span className="chip">{k < 0 ? "ground control station" : SCREENS[k].tag}</span>
    </div>
  );
}

function Footage({ src, tag }: { src: string; tag: string }) {
  return (
    <div className="footage">
      <video src={src} autoPlay muted loop playsInline preload="none" />
      <span className="chip">{tag}</span>
    </div>
  );
}

const N = CARDS.length;
// Cumulative scroll windows: card k runs from EDGE[k] to EDGE[k + 1], as fractions of the scene.
const WINDOWS = CARDS.reduce((w, c) => w + (c.span ?? 1), 0);
const EDGE = CARDS.reduce<number[]>(
  (e, c) => [...e, e[e.length - 1] + (c.span ?? 1) / WINDOWS],
  [0],
);
const at = (k: number) => `${EDGE[k] + 0.005},${k === N - 1 ? 1.01 : EDGE[k + 1] - 0.005}`;

/** Armory's card grammar: a pinned frame, copy left, a live panel right, one card per scroll window. */
export function Cards() {
  const root = useRef<HTMLDivElement>(null);
  // Per-card updaters for the panels that step through data rather than clip it.
  // A plain array: written from child effects, read from the scroll callback.
  const binders = useMemo<((lp: number) => void)[]>(() => [], []);
  const onP = (p: number) => {
    let k = 0;
    while (k < N - 1 && p >= EDGE[k + 1]) k++;
    const lp = Math.min(1, (p - EDGE[k]) / (EDGE[k + 1] - EDGE[k]));
    root.current?.style.setProperty("--lp", lp.toFixed(4));
    root.current?.style.setProperty("--prog", p.toFixed(4));
    binders[k]?.(lp);
  };
  return (
    <Scene vh={WINDOWS * 150} map={null} opaque stage="cards-stage" onP={onP} id="proof">
      <div className="cards" ref={root}>
        <div className="card-left">
          {CARDS.map((c, k) => (
            <div key={c.key} className="card-copy" data-at={at(k)}>
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
            <div key={c.key} className="card-visual" data-at={at(k)}>
              {c.visual((f) => void (binders[k] = f))}
            </div>
          ))}
        </div>
      </div>
    </Scene>
  );
}
