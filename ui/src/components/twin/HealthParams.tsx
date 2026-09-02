/**
 * The health parameters the filter is carrying, and where each has been.
 *
 * Nothing here is measured. These are the states the UKF added to the engine
 * model so that a disagreement has somewhere to go, and the whole panel is inside
 * one dashed border with one `◊ INFERRED` tag rather than tagging each row: the
 * distinction being drawn is between this panel and every other panel on the
 * screen, not between rows inside it.
 *
 * # The micro-chart is the evidence, not decoration
 *
 * A number beside a name says a parameter is at 0.81. The trace beside it says
 * whether it arrived there over twenty minutes or was there when the engine
 * started, and those are different findings: the first is degradation and the
 * second is a machine that was already worn when the monitor was fitted. It is a
 * sparkline against its own excursion rather than against the failure threshold,
 * because the threshold view already exists on ANALYSIS at full size, and drawn
 * against a threshold most of these are flat lines at the top of the box.
 *
 * The declining mark is taken from the prognosis rather than from the shape of
 * the trace, so it says what the fitted slope says. That fit needs five minutes
 * of history, and until it has them every row correctly reads steady.
 */

import { useRef } from "react";

import { fmt, NO_VALUE } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import type { Parameter } from "@/lib/signatures";
import { isFresh } from "@/lib/telemetry";
import { telemetry } from "@/store/telemetry";

/** Sparkline coordinate space, mapped to the row with `preserveAspectRatio`. */
const BOX_W = 100;
const BOX_H = 24;

/** Most points drawn per sparkline. The box is ~90 px wide. */
const MAX_POINTS = 90;

/**
 * Narrowest vertical span a sparkline will scale to, as a fraction of the
 * parameter's nominal-to-failure range.
 *
 * Without a floor the estimator's own jitter is autoscaled to fill the box and
 * every healthy parameter draws as a seismograph. Two percent of the range is
 * about the size of that jitter, so noise stays flat and a real decline moves.
 */
const MIN_SPAN = 0.02;

export function HealthParams({ parameters }: { parameters: Parameter[] }) {
  const values = useRef<(HTMLSpanElement | null)[]>([]);
  const marks = useRef<(HTMLSpanElement | null)[]>([]);
  const names = useRef<(HTMLSpanElement | null)[]>([]);
  const paths = useRef<(SVGPathElement | null)[]>([]);
  const block = useRef<HTMLDivElement>(null);
  const staleRef = useRef<HTMLSpanElement>(null);

  useLiveSink((frame) => {
    const twin = frame.twin;
    const live = twin !== null && isFresh(frame.ages.engine_ms);

    // The traces keep their history, the same way the paired cells do, and the
    // same way they must not keep looking current: a thirty minute trajectory
    // drawn at full strength beside a dashed value is an old inference presented
    // as the present one. Dimming the whole block rather than each path, because
    // every row goes stale together.
    if (staleRef.current) staleRef.current.hidden = live;
    block.current?.toggleAttribute("data-stale", !live);

    for (let i = 0; i < parameters.length; i += 1) {
      const descriptor = parameters[i]!;
      const value = values.current[i];
      const mark = marks.current[i];
      const name = names.current[i];
      const path = paths.current[i];

      const theta = live ? (twin.theta[i] ?? Number.NaN) : Number.NaN;
      if (value) value.textContent = Number.isFinite(theta) ? fmt(theta, 3) : NO_VALUE;

      // Declining per the fitted trend, which is also what gives it a remaining
      // life. A parameter with no fit yet is steady, not falling, and a fit from
      // a feed that has since died is not current: gated on `live` alongside the
      // value, or a row shows an accented falling arrow beside a dash.
      const rul = live ? frame.prognosis?.parameter[i] : undefined;
      const declining = rul !== undefined && rul.hours !== null;
      value?.toggleAttribute("data-alarm", declining);
      name?.toggleAttribute("data-alarm", declining);
      if (mark) {
        mark.textContent = declining ? "↓" : "→";
        mark.toggleAttribute("data-alarm", declining);
      }

      if (!path) continue;
      const ring = telemetry.theta.ring(i);
      const view = ring?.view();
      if (!view || view.length < 2) {
        path.setAttribute("d", "");
        continue;
      }

      let min = Infinity;
      let max = -Infinity;
      for (const v of view) {
        if (v < min) min = v;
        if (v > max) max = v;
      }
      const range = Math.abs(descriptor.nominal - descriptor.failure) || 1;
      const span = Math.max(max - min, range * MIN_SPAN);
      const mid = (min + max) / 2;
      const lo = mid - span / 2;

      const step = Math.max(1, Math.ceil(view.length / MAX_POINTS));
      let d = "";
      for (let k = 0; k < view.length; k += step) {
        const x = ((k / (view.length - 1)) * BOX_W).toFixed(1);
        const y = (BOX_H - ((view[k]! - lo) / span) * BOX_H).toFixed(1);
        d += `${d ? "L" : "M"}${x} ${y}`;
      }
      const last = view[view.length - 1]!;
      d += `L${BOX_W} ${(BOX_H - ((last - lo) / span) * BOX_H).toFixed(1)}`;
      path.setAttribute("d", d);
    }
  });

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <div className="border-border flex h-[36px] shrink-0 items-center justify-between gap-[10px] border-b px-4">
        <span className="t-section truncate whitespace-nowrap">
          EST. HEALTH PARAMS
          <span
            ref={staleRef}
            hidden
            className="text-primary ml-[8px] text-[10px] tracking-[0.08em]"
          >
            · STALE
          </span>
        </span>
        <span className="border-structure-hi text-foreground-dim shrink-0 border border-dashed px-[7px] py-[3px] text-[10px] tracking-[0.08em]">
          ◊ INFERRED
        </span>
      </div>
      <div className="flex min-h-0 flex-1 p-3">
        <div ref={block} className="inferred flex min-h-0 flex-1 flex-col px-[10px]">
          {parameters.map((p, i) => (
            <div
              key={p.name}
              // The floor is what a 12px name needs and no more. Ten rows at 30
              // sum past the panel's share as soon as the window is shorter than
              // 900 and the last parameter is clipped off the bottom, which on
              // this panel means a health parameter silently ceasing to exist.
              className="border-border flex min-h-[22px] flex-1 items-center gap-2 border-t first:border-t-0"
            >
              <span
                ref={(el) => {
                  names.current[i] = el;
                }}
                className="min-w-0 flex-1 truncate text-[12px]"
              >
                {p.name}
              </span>
              <svg
                viewBox={`0 0 ${BOX_W} ${BOX_H}`}
                preserveAspectRatio="none"
                className="h-[18px] w-[76px] shrink-0"
                aria-hidden="true"
              >
                <path
                  ref={(el) => {
                    paths.current[i] = el;
                  }}
                  d=""
                  fill="none"
                  stroke="var(--foreground-dim)"
                  strokeWidth={1.25}
                  vectorEffect="non-scaling-stroke"
                />
              </svg>
              <span className="flex shrink-0 items-baseline gap-2">
                <span
                  ref={(el) => {
                    values.current[i] = el;
                  }}
                  className="num text-[14px]"
                >
                  {NO_VALUE}
                </span>
                <span
                  ref={(el) => {
                    marks.current[i] = el;
                  }}
                  className="text-foreground-dim w-[10px] text-center text-[11px]"
                >
                  →
                </span>
              </span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
