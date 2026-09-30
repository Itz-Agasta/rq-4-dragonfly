import { useEffect, useRef } from "react";

import coking from "../data/coking.json";
import rul from "../data/rul.json";

// Every chart here draws a mission recording exported by site/pipeline/export_traces.py
// from `just mission` Parquet. Annotations are read from the same file, never typed.

const W = 800;
const H = 520;
const L = 64; // left edge of every plot area
const R = 772;

type Pt = [number, number];
const path = (pts: Pt[]) =>
  pts.map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(1)},${y.toFixed(1)}`).join("");
const scale = (d0: number, d1: number, r0: number, r1: number) => (v: number) =>
  r0 + ((v - d0) / (d1 - d0)) * (r1 - r0);

/** Reveal left to right: the plot area is clipped to the card's progress. */
function Reveal({ id, p }: { id: string; p: string }) {
  return (
    <clipPath id={id}>
      <rect
        x={0}
        y={0}
        height={H}
        style={{ width: `calc(${L}px + (${R - L}px * ${p}))` } as React.CSSProperties}
      />
    </clipPath>
  );
}

const A = coking.alarms;
const T0 = 60;
const T1 = 360;

/** Detect: EGT 3 measured against the twin, and the CUSUM crossing its decision interval. */
export function DetectChart() {
  const tr = coking.trace;
  const x = scale(T0, T1, L, R);
  const keep = tr.t
    .map((t, i) => [t, i] as const)
    .filter(([t]) => t !== null && t >= T0 && t <= T1);
  const egt = keep
    .map(([, i]) => tr.egt_3[i])
    .concat(keep.map(([, i]) => tr.egt_3_hat[i]))
    .filter((v): v is number => v !== null);
  const yE = scale(Math.min(...egt) - 8, Math.max(...egt) + 8, 230, 48);
  const lim = tr.cusum_limit ?? 5;
  const yC = scale(0, lim * 3, 470, 300);
  const line = (key: "egt_3" | "egt_3_hat") =>
    path(keep.filter(([, i]) => tr[key][i] !== null).map(([t, i]) => [x(t!), yE(tr[key][i]!)]));
  const cus = path(
    keep
      .filter(([, i]) => tr.cusum[i] !== null)
      .map(([t, i]) => [x(t!), yC(Math.min(tr.cusum[i]!, lim * 3))]),
  );
  const mark = (t: number, label: string, cls: string, dy: number, left = false) => (
    <g className={cls}>
      <line x1={x(t)} x2={x(t)} y1={36} y2={478} />
      <text x={x(t) + (left ? -6 : 6)} y={dy} textAnchor={left ? "end" : "start"}>
        {label}
      </text>
    </g>
  );
  return (
    <svg className="chart" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid meet">
      <Reveal id="rv-detect" p="var(--lp, 0)" />
      <text className="ax" x={L} y={28}>
        EGT 3, K
      </text>
      <text className="ax" x={L} y={292}>
        CUSUM
      </text>
      <line className="limit" x1={L} x2={R} y1={yC(lim)} y2={yC(lim)} />
      <text className="limit-t" x={R} y={yC(lim) - 6} textAnchor="end">
        decision interval {lim}
      </text>
      <g clipPath="url(#rv-detect)">
        <path className="twin" d={line("egt_3_hat")} />
        <path className="meas" d={line("egt_3")} />
        <path className="meas" d={cus} />
        {mark(coking.onset_s, `fault onset T+${coking.onset_s}`, "m-dim", 262, true)}
        {A.cusum_s &&
          mark(A.cusum_s, `CUSUM T+${A.cusum_s.toFixed(1)} · ${A.cusum_channel}`, "m-hot", 318)}
        {A.mahalanobis_s &&
          mark(A.mahalanobis_s, `Mahalanobis T+${A.mahalanobis_s.toFixed(1)}`, "m-ink", 90)}
      </g>
      <text className="never" x={R} y={500} textAnchor="end">
        certified redline: {A.redline ? "tripped" : "never"}
      </text>
      <text className="ax" x={L} y={500}>
        T+{T0} s to T+{T1} s
      </text>
    </svg>
  );
}

const post = coking.posterior;
const onsetIdx = post.t.findIndex((t) => t !== null && t >= coking.onset_s);

/** Diagnose: the posterior over the nine hypotheses, stepped through the mission by scroll. */
export function DiagnoseChart({ bind }: { bind: (f: (lp: number) => void) => void }) {
  const bars = useRef<(SVGRectElement | null)[]>([]);
  const vals = useRef<(SVGTextElement | null)[]>([]);
  const clock = useRef<SVGTextElement>(null);
  const x0 = 336;
  const x1 = 704;
  useEffect(
    () =>
      bind((lp) => {
        const i = Math.round(onsetIdx + lp * (post.t.length - 1 - onsetIdx));
        const p = post.p[i];
        const top = p.indexOf(Math.max(...p.map((v) => v ?? 0)));
        p.forEach((v, k) => {
          const w = Math.max(1, (v ?? 0) * (x1 - x0));
          bars.current[k]?.setAttribute("width", w.toFixed(1));
          bars.current[k]?.setAttribute("class", k === top && (v ?? 0) > 0.5 ? "bar hot" : "bar");
          if (vals.current[k]) vals.current[k]!.textContent = `${Math.round((v ?? 0) * 100)}%`;
        });
        if (clock.current) clock.current.textContent = `T+${Math.round(post.t[i] ?? 0)} s`;
      }),
    [bind],
  );
  return (
    <svg className="chart" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid meet">
      <text className="ax" x={L} y={40}>
        posterior, 9 hypotheses from 22 residuals
      </text>
      <text className="clock" x={R} y={40} textAnchor="end" ref={clock}>
        T+90 s
      </text>
      {post.names.map((n, k) => (
        <g key={n} transform={`translate(0 ${80 + k * 44})`}>
          <text className="row" x={L} y={16}>
            {n}
          </text>
          <rect className="track" x={x0} y={4} width={x1 - x0} height={14} />
          <rect
            className="bar"
            x={x0}
            y={4}
            width={1}
            height={14}
            ref={(el) => void (bars.current[k] = el)}
          />
          <text
            className="val"
            x={R}
            y={16}
            textAnchor="end"
            ref={(el) => void (vals.current[k] = el)}
          >
            0%
          </text>
        </g>
      ))}
    </svg>
  );
}

/** Predict: injector 3's discharge coefficient, its 1-sigma band, and the projection to failure. */
export function PredictChart() {
  const end = rul.t_h[rul.t_h.length - 1] ?? 1;
  const x = scale(0, end + (rul.p90_h ?? 3) + 0.3, L, R);
  const y = scale(0.55, 1.0, 470, 48);
  const pts = rul.t_h
    .map((t, i) => [t, rul.theta[i], rul.sigma[i]] as const)
    .filter(([t, v, s]) => t !== null && v !== null && s !== null) as [number, number, number][];
  const band =
    path(pts.map(([t, v, s]) => [x(t), y(v + s)])) +
    pts
      .slice()
      .reverse()
      .map(([t, v, s]) => `L${x(t).toFixed(1)},${y(v - s).toFixed(1)}`)
      .join("") +
    "Z";
  const [lt, lv] = pts[pts.length - 1];
  const f = rul.failure;
  const fan = `M${x(lt)},${y(lv)}L${x(lt + rul.p10_h!)},${y(f)}L${x(lt + rul.p90_h!)},${y(f)}Z`;
  return (
    <svg className="chart" viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid meet">
      <Reveal id="rv-predict" p="min(1, var(--lp, 0) * 1.4)" />
      <text className="ax" x={L} y={28}>
        injector 3 discharge coefficient
      </text>
      <line className="nominal" x1={L} x2={R} y1={y(rul.nominal)} y2={y(rul.nominal)} />
      <text className="ax" x={R} y={y(rul.nominal) - 6} textAnchor="end">
        nominal {rul.nominal}
      </text>
      <line className="limit crit" x1={L} x2={R} y1={y(f)} y2={y(f)} />
      <text className="limit-t crit" x={L} y={y(f) + 18}>
        failure {f}
      </text>
      <g clipPath="url(#rv-predict)">
        <path className="band" d={band} />
        <path className="meas" d={path(pts.map(([t, v]) => [x(t), v === null ? 0 : y(v)]))} />
        <path className="fan" d={fan} />
        <line className="proj" x1={x(lt)} y1={y(lv)} x2={x(lt + rul.rul_h!)} y2={y(f)} />
        <g transform={`translate(${x(lt + rul.rul_h!)} ${y(f) - 64})`} textAnchor="middle">
          <text className="big">{rul.rul_h} h</text>
          <text className="ax" y={22}>
            p10 {rul.p10_h} · p90 {rul.p90_h}
          </text>
        </g>
      </g>
      <text className="ax" x={L} y={500}>
        mission hours, then projected
      </text>
    </svg>
  );
}
