/**
 * The degradation trajectory: where the limiting health parameter has been, and
 * where the fit says it is going.
 *
 * Nothing on this chart is a measurement. The history is the filter's estimate of
 * a constant inside the engine and the forecast is a line fitted through it, so
 * the panel carries one `◊ INFERRED` tag rather than trying to distinguish
 * degrees of inference inside the plot. Provenance stays typographic: solid for
 * what has happened, dashed for what has not, and no hue on either.
 *
 * The cone is drawn between the line that reaches failure at p10 and the line
 * that reaches it at p90, so it is exactly as wide as the interval printed above
 * it. A hairline cone drawn beside a stated multi-hour spread reads as false
 * precision and is worse than showing no cone at all.
 */

import { useRef } from "react";

import { focus } from "@/components/analysis/prognosis";
import { fmt, missionClock, NO_VALUE } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import type { Parameter } from "@/lib/signatures";
import { isFresh } from "@/lib/telemetry";
import { telemetry } from "@/store/telemetry";

/** Plot coordinate space. Mapped to the box with `preserveAspectRatio="none"`. */
const BOX = 1000;

/**
 * Most history points drawn.
 *
 * The plot is around 300 px wide, so 180 points is already sub-pixel. Decimating
 * keeps the path string short enough to rebuild every animation frame, which it
 * has to be: the x axis rescales whenever the forecast does.
 */
const MAX_POINTS = 180;

/** Furthest ahead the forecast is drawn, seconds. */
const MAX_AHEAD_S = 12 * 3600;

/** Headroom above nominal and below failure, as a fraction of the span. */
const PAD_TOP = 0.08;
const PAD_BOTTOM = 0.12;

export function Trajectory({ parameters }: { parameters: Parameter[] }) {
  const cone = useRef<SVGPolygonElement>(null);
  const forecast = useRef<SVGPathElement>(null);
  const history = useRef<SVGPathElement>(null);
  const fail = useRef<SVGLineElement>(null);
  const now = useRef<SVGLineElement>(null);
  const failLabel = useRef<HTMLSpanElement>(null);
  const top = useRef<HTMLSpanElement>(null);
  const name = useRef<HTMLSpanElement>(null);
  const value = useRef<HTMLSpanElement>(null);
  const left = useRef<HTMLSpanElement>(null);
  const middle = useRef<HTMLSpanElement>(null);
  const right = useRef<HTMLSpanElement>(null);

  useLiveSink((frame) => {
    const blank = () => {
      if (history.current) history.current.setAttribute("d", "");
      if (forecast.current) forecast.current.setAttribute("d", "");
      if (cone.current) cone.current.setAttribute("points", "");
      if (now.current) now.current.setAttribute("x1", "-10");
      if (now.current) now.current.setAttribute("x2", "-10");
    };

    const i = isFresh(frame.ages.engine_ms) ? focus(frame, parameters) : null;
    const descriptor = i === null ? undefined : parameters[i];
    const ring = i === null ? null : telemetry.theta.ring(i);
    if (i === null || !descriptor || !ring) {
      blank();
      if (name.current) name.current.textContent = NO_VALUE;
      [top, value, failLabel, left, middle, right].forEach((slot) => {
        if (slot.current) slot.current.textContent = "";
      });
      return;
    }

    const times = telemetry.theta.time.view();
    const values = ring.view();
    if (name.current) name.current.textContent = descriptor.name;
    if (top.current) top.current.textContent = `NOM ${fmt(descriptor.nominal, 2)}`;
    if (failLabel.current) failLabel.current.textContent = `FAIL ${fmt(descriptor.failure, 2)}`;
    if (value.current) {
      value.current.textContent = values.length
        ? `NOW ${fmt(values[values.length - 1]!, 3)}`
        : NO_VALUE;
    }
    if (values.length < 2) {
      blank();
      if (middle.current) middle.current.textContent = "building the trend window";
      if (left.current) left.current.textContent = "";
      if (right.current) right.current.textContent = "";
      return;
    }

    const rul = frame.prognosis?.parameter[i];
    const hours = rul?.hours ?? null;
    const p10 = rul?.p10 ?? null;
    const p90 = rul?.p90 ?? null;

    // Forward span. Far enough to show the interval when there is one, and zero
    // when there is nothing to project, which leaves a history-only plot rather
    // than a forecast into an arbitrary future.
    const aheadH = hours === null ? 0 : (p90 ?? hours * 2);
    const aheadS = Math.min(Math.max(aheadH * 3600, hours === null ? 0 : 60), MAX_AHEAD_S);

    const tNow = times[times.length - 1]!;
    const available = tNow - times[0]!;
    // History is shown against the forecast rather than at full length: thirty
    // minutes of flat estimate beside a nine-minute projection hides the only
    // part of the plot anyone is looking at.
    const backS = aheadS > 0 ? Math.min(available, Math.max(aheadS, 120)) : available;
    const t0 = tNow - backS;
    const tSpan = backS + aheadS || 1;
    const x = (t: number) => ((t - t0) / tSpan) * BOX;

    const range = descriptor.nominal - descriptor.failure || 1;
    let ceiling = descriptor.nominal + range * PAD_TOP;
    for (const v of values) ceiling = Math.max(ceiling, v + range * PAD_TOP * 0.5);
    const floor = descriptor.failure - range * PAD_BOTTOM;
    const y = (v: number) => ((ceiling - v) / (ceiling - floor)) * BOX;

    // Decimated by index rather than by time, which is safe because the ring is
    // already uniform at 1 Hz.
    const step = Math.max(1, Math.ceil(values.length / MAX_POINTS));
    let path = "";
    for (let k = 0; k < values.length; k += step) {
      const t = times[k];
      const v = values[k];
      if (t === undefined || v === undefined) continue;
      path += `${path ? "L" : "M"}${x(t).toFixed(1)} ${y(v).toFixed(1)}`;
    }
    path += `L${x(tNow).toFixed(1)} ${y(values[values.length - 1]!).toFixed(1)}`;
    if (history.current) history.current.setAttribute("d", path);

    const yFail = y(descriptor.failure);
    if (fail.current) {
      fail.current.setAttribute("y1", yFail.toFixed(1));
      fail.current.setAttribute("y2", yFail.toFixed(1));
    }
    // The rail shares the plot's vertical padding, so the same fraction places a
    // label against the line it names.
    const rail = (el: HTMLSpanElement | null, v: number) => {
      if (el) el.style.top = `calc(8px + ${(y(v) / BOX) * 100}% - ${(16 * y(v)) / BOX}px)`;
    };
    const current = values[values.length - 1]!;
    rail(top.current, descriptor.nominal);
    rail(value.current, current);
    rail(failLabel.current, descriptor.failure);
    // On a healthy engine the current value sits on nominal and the two labels
    // land on top of each other. The live one wins; the axis top is the less
    // useful of the two and is already implied by where the trace starts.
    if (top.current) {
      const collides = Math.abs(y(descriptor.nominal) - y(current)) < BOX * 0.07;
      top.current.style.visibility = collides ? "hidden" : "visible";
    }
    const xNow = x(tNow);
    if (now.current) {
      now.current.setAttribute("x1", xNow.toFixed(1));
      now.current.setAttribute("x2", xNow.toFixed(1));
    }

    if (left.current) left.current.textContent = `−${fmt(backS / 60, 0)} min`;
    if (middle.current) middle.current.textContent = missionClock(tNow);

    if (hours === null || aheadS === 0) {
      if (forecast.current) forecast.current.setAttribute("d", "");
      if (cone.current) cone.current.setAttribute("points", "");
      if (right.current) right.current.textContent = "no decline";
      return;
    }

    const v0 = values[values.length - 1]!;
    // An unbounded upper bound is drawn at the horizon rather than off the plot.
    const cross = (h: number | null) => x(tNow + Math.min((h ?? Infinity) * 3600, MAX_AHEAD_S));
    if (forecast.current) {
      forecast.current.setAttribute(
        "d",
        `M${xNow.toFixed(1)} ${y(v0).toFixed(1)}L${cross(hours).toFixed(1)} ${yFail.toFixed(1)}`,
      );
    }
    if (cone.current) {
      // Both edges leave the same point and reach the failure rule at the two
      // bounds, so the cone's width at the rule is the interval itself.
      cone.current.setAttribute(
        "points",
        `${xNow.toFixed(1)},${y(v0).toFixed(1)} ${cross(p10).toFixed(1)},${yFail.toFixed(1)} ${cross(p90).toFixed(1)},${yFail.toFixed(1)}`,
      );
    }
    if (right.current) {
      right.current.textContent =
        aheadH >= 1 ? `+${fmt(aheadH, 1)} h` : `+${fmt(aheadH * 60, 0)} min`;
    }
  });

  return (
    <div className="border-border flex max-h-[300px] min-h-[170px] flex-1 flex-col overflow-hidden border-t">
      <div className="border-border flex h-[34px] shrink-0 items-center justify-between gap-2 border-b px-4">
        <span className="flex min-w-0 items-baseline gap-2">
          <span ref={name} className="text-foreground truncate text-[12px]">
            {NO_VALUE}
          </span>
          <span className="label-micro shrink-0">health param</span>
        </span>
        <span className="border-structure-hi text-foreground-dim shrink-0 border border-dashed px-[6px] py-[2px] text-[10px] tracking-[0.08em]">
          ◊ INFERRED
        </span>
      </div>

      <div className="flex min-h-0 min-w-0 flex-1">
        <div className="min-h-0 min-w-0 flex-1 py-2 pr-2 pl-4">
          <svg
            viewBox={`0 0 ${BOX} ${BOX}`}
            preserveAspectRatio="none"
            className="block h-full w-full"
            aria-hidden="true"
          >
            <line
              x1={0}
              x2={BOX}
              y1={BOX * 0.25}
              y2={BOX * 0.25}
              stroke="var(--grid)"
              strokeWidth={1}
              vectorEffect="non-scaling-stroke"
            />
            <line
              x1={0}
              x2={BOX}
              y1={BOX * 0.5}
              y2={BOX * 0.5}
              stroke="var(--grid)"
              strokeWidth={1}
              vectorEffect="non-scaling-stroke"
            />
            <polygon ref={cone} points="" fill="var(--predicted)" fillOpacity={0.13} />
            <line
              ref={now}
              x1={-10}
              x2={-10}
              y1={0}
              y2={BOX}
              stroke="var(--structure)"
              strokeWidth={1}
              strokeDasharray="3 3"
              vectorEffect="non-scaling-stroke"
            />
            <path
              ref={forecast}
              d=""
              fill="none"
              stroke="var(--predicted)"
              strokeWidth={1.5}
              strokeDasharray="6 4"
              vectorEffect="non-scaling-stroke"
            />
            <path
              ref={history}
              d=""
              fill="none"
              stroke="var(--foreground)"
              strokeWidth={2}
              vectorEffect="non-scaling-stroke"
            />
            {/* The one place red is legitimate on this screen: it is the value at
                which the subsystem stops meeting its duty, not a reading. */}
            <line
              ref={fail}
              x1={0}
              x2={BOX}
              y1={-10}
              y2={-10}
              stroke="var(--crit)"
              strokeWidth={1.5}
              vectorEffect="non-scaling-stroke"
            />
          </svg>
        </div>

        {/* A rail beside the plot rather than callouts over it: floating labels
            collide with whatever the panel below starts with as soon as the
            chart is short. Each label sits at the height of the line it names,
            which is the only arrangement in which the rail says anything. */}
        <div className="relative w-[74px] shrink-0 py-2 pr-3">
          <span
            ref={top}
            className="num text-muted-foreground absolute right-3 -translate-y-1/2 text-[10px]"
          />
          <span
            ref={value}
            className="num text-foreground absolute right-3 -translate-y-1/2 text-[10px] whitespace-nowrap"
          />
          <span
            ref={failLabel}
            className="num text-crit absolute right-3 -translate-y-1/2 text-[10px] whitespace-nowrap"
          />
        </div>
      </div>

      <div className="flex shrink-0 justify-between px-4 pb-2">
        <span ref={left} className="label-micro" />
        <span ref={middle} className="label-micro" />
        <span ref={right} className="label-micro" />
      </div>
    </div>
  );
}
