/**
 * One streaming telemetry strip.
 *
 * uPlot, fed directly from the ring buffers by the shared render loop. React
 * renders this component once; every subsequent update is a `setData` call on a
 * canvas. Charting libraries that re-render through React cannot hold 20 Hz
 * across six strips without dropping frames.
 *
 * Shared between OPS and, later, REPLAY, which is why the series are described by
 * channel id rather than hard-coded here.
 */

import { useEffect, useRef } from "react";
import uPlot from "uplot";

import "uplot/dist/uPlot.min.css";

import { fmt, grouped, NO_VALUE } from "@/lib/fmt";
import { subscribe } from "@/lib/live";
import { type Frame, isFresh } from "@/lib/telemetry";
import { channel } from "@/store/frame";
import { HISTORY_SECONDS, telemetry } from "@/store/telemetry";

/** How a series is drawn. Weight and dash, never hue. */
export type Emphasis = "measured" | "ghost" | "accent";

export interface StripSeries {
  /** Channel id, as registered in the channel registry. */
  id: string;
  emphasis?: Emphasis;
}

export interface StripProps {
  /** Micro-label, top left. */
  title: string;
  /** Channel whose current value is shown top right. */
  readout: string;
  series: StripSeries[];
  /**
   * Cursor sync group. Every strip sharing a key shares a hover cursor, which is
   * what lets an operator read the same instant across six channels.
   */
  syncKey: string;
  /**
   * Narrowest y span the plot will scale to, in the channel's own units.
   *
   * Without a floor, autoscaling a channel that is holding steady expands its
   * sensor noise to fill the panel, and an engine sitting at 3,720 rpm plus or
   * minus three reads as one that is thrashing. The floor is what makes a quiet
   * channel look quiet and leaves room for a real excursion to stand out.
   */
  minSpan: number;

  /**
   * Whether the readout is currently alarming, evaluated per frame.
   *
   * A predicate rather than a flag so the accent is earned by the live data. A
   * channel painted as degrading while its trace sits on top of its neighbours
   * is worse than no accent at all.
   */
  alarmWhen?: (frame: Frame) => boolean;
}

/**
 * Resolve a theme token to a concrete colour.
 *
 * uPlot hands `stroke` straight to the canvas 2D context, and `strokeStyle` does
 * not understand `var(--x)` — it silently ignores the assignment and keeps the
 * previous value, so a trace styled with a CSS variable draws in black on black
 * and looks like no data at all. SVG resolves variables and canvas does not; this
 * is the seam between them.
 */
function token(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

const STROKE: Record<Emphasis, { token: string; width: number }> = {
  measured: { token: "--measured", width: 2 },
  ghost: { token: "--trace-ghost", width: 1.25 },
  accent: { token: "--primary", width: 2 },
};

/** Vertical fraction of the plot the trace is allowed to fill. */
const FILL = 0.76;

/**
 * Autoscale to the data, never to zero, leaving headroom top and bottom.
 *
 * Scaling to zero flattens a cruise segment into a straight line and throws away
 * the variation that is the entire point of a strip chart.
 */
function paddedRange(min: number, max: number, minSpan: number): [number, number] {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return [0, minSpan];
  const mid = (min + max) / 2;
  const span = Math.max(max - min, minSpan) / FILL;
  return [mid - span / 2, mid + span / 2];
}

/** Three hairlines at the quarters, matching the schematic's grid weight. */
function drawGrid(u: uPlot): void {
  const { ctx } = u;
  const { left, top, width, height } = u.bbox;
  ctx.save();
  ctx.strokeStyle = token("--grid") || "#131315";
  ctx.lineWidth = 1;
  for (const f of [0.25, 0.5, 0.75]) {
    const y = Math.round(top + height * f) + 0.5;
    ctx.beginPath();
    ctx.moveTo(left, y);
    ctx.lineTo(left + width, y);
    ctx.stroke();
  }
  ctx.restore();
}

export function Strip({ title, readout, series, syncKey, minSpan, alarmWhen }: StripProps) {
  const host = useRef<HTMLDivElement>(null);
  const valueRef = useRef<HTMLSpanElement>(null);
  const staleRef = useRef<HTMLSpanElement>(null);
  const plot = useRef<uPlot | null>(null);

  useEffect(() => {
    const el = host.current;
    if (!el) return;

    const chart = new uPlot(
      {
        width: el.clientWidth || 1,
        height: el.clientHeight || 1,
        padding: [4, 0, 2, 0],
        legend: { show: false },
        axes: [{ show: false }, { show: false }],
        scales: {
          x: { time: false },
          y: { range: (_u, min, max) => paddedRange(min, max, minSpan) },
        },
        cursor: {
          sync: { key: syncKey, scales: ["x", null] },
          drag: { x: false, y: false },
          points: { show: false },
          y: false,
        },
        series: [
          {},
          ...series.map((s) => {
            const style = STROKE[s.emphasis ?? "measured"];
            return {
              stroke: token(style.token),
              width: style.width,
              points: { show: false },
            };
          }),
        ],
        hooks: { drawClear: [drawGrid] },
      },
      [new Float64Array(0), ...series.map(() => new Float64Array(0))] as uPlot.AlignedData,
      el,
    );
    plot.current = chart;

    const resize = new ResizeObserver(([entry]) => {
      const box = entry?.contentRect;
      if (box && box.width > 0 && box.height > 0) {
        chart.setSize({ width: box.width, height: box.height });
      }
    });
    resize.observe(el);

    // Only redraw when a frame actually arrived. The display refreshes three
    // times per telemetry frame, and rescaling 1,800 samples per series for data
    // that has not changed is the difference between 6% and 20% of a core.
    let seen = -1;
    const stop = subscribe((frame) => {
      if (frame.seq === seen) return;
      seen = frame.seq;
      chart.setData([
        telemetry.time.view(),
        ...series.map((s) => telemetry.ring(s.id).view()),
      ] as uPlot.AlignedData);
    });

    return () => {
      stop();
      resize.disconnect();
      chart.destroy();
      plot.current = null;
    };
  }, [series, syncKey, minSpan]);

  const ch = channel(readout);

  useEffect(() => {
    return subscribe((frame) => {
      const el = valueRef.current;
      const flag = staleRef.current;
      if (flag) {
        // Dimming alone is not enough. A frozen trace looks exactly like a steady
        // one, and an operator reading a held value as a live one is a safety
        // failure rather than a cosmetic one, so the strip says the word.
        flag.hidden = isFresh(frame.ages[ch.source]);
      }
      if (!el) return;
      const v = ch.get(frame);
      const text = ch.dp === 0 && Math.abs(v) >= 1000 ? grouped(v) : fmt(v, ch.dp);
      if (el.textContent !== text) el.textContent = text;
      const stale = !isFresh(frame.ages[ch.source]);
      if (stale !== el.hasAttribute("data-stale")) el.toggleAttribute("data-stale", stale);
      const alarm = alarmWhen?.(frame) ?? false;
      if (alarm !== el.hasAttribute("data-alarm")) el.toggleAttribute("data-alarm", alarm);
    });
  }, [ch, alarmWhen]);

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col px-4 py-[10px]">
      <div className="flex items-baseline justify-between gap-[10px]">
        <span className="label-micro whitespace-nowrap">
          {title}
          <span className="text-structure mx-[6px]">·</span>
          {HISTORY_SECONDS} s
          <span ref={staleRef} hidden className="text-primary ml-[6px]">
            · STALE
          </span>
        </span>
        <span className="num text-[16px] leading-none whitespace-nowrap">
          <span
            ref={valueRef}
            className="data-[alarm]:text-primary data-[stale]:text-foreground-dim"
          >
            {NO_VALUE}
          </span>
          <span className="text-muted-foreground ml-1 text-[10px]">{ch.unit}</span>
        </span>
      </div>
      <div ref={host} className="mt-[6px] min-h-0 min-w-0 flex-1" />
    </div>
  );
}
