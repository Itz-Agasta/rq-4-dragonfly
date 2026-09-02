/**
 * One channel, measured against the physics, with its residual beneath.
 *
 * Two uPlot instances on one time base: the trace pair on top, the residual in
 * standard deviations below. They are separate charts rather than one chart with
 * two scales because the residual's y axis is fixed by the tolerance band and the
 * trace's is not, and because a residual drawn as a filled area under a trace
 * reads as a second signal rather than as the gap between the first two.
 *
 * # The band is drawn as a region, not as two rules
 *
 * A residual strip scaled to a fixed plus or minus four sigma draws a healthy
 * channel as a flat line, which throws away the fact that it is wandering inside
 * the tolerance rather than sitting on it. So the y scale follows the data, with
 * a floor of one sigma, and the plus or minus three sigma band is painted as a
 * region: on a quiet channel it fills the strip, which says "entirely inside
 * tolerance", and as a residual grows the band shrinks into a visible envelope
 * with the excursion crossing it. Only the part outside the band is accented.
 */

import { useEffect, useRef } from "react";
import uPlot from "uplot";

import { fmt, grouped, NO_VALUE, signed } from "@/lib/fmt";
import { subscribe, useLiveSink } from "@/lib/live";
import { isFresh } from "@/lib/telemetry";
import { token } from "@/lib/token";
import { COMPARED } from "@/store/compared";
import { telemetry } from "@/store/telemetry";
import { BAND_SIGMA } from "@/store/twin";

/** Narrowest residual scale, in sigma. Keeps a quiet channel visibly alive. */
const RESIDUAL_FLOOR = 1;

/**
 * Headroom above the largest residual in the window.
 *
 * Generous on purpose. A residual sitting at a constant offset, which is what a
 * settled fault looks like, otherwise lands hard against the top or bottom edge
 * where its own wander is a pixel tall and there is no room to read it against
 * zero. At 1.6 a channel at -2.5 sigma sits around three fifths of the way down
 * with the band edge visible below it, which is the reading someone needs.
 */
const RESIDUAL_HEADROOM = 1.6;

/** Vertical fraction of the trace plot the two traces are allowed to fill. */
const TRACE_FILL = 0.76;

export interface PairedCellProps {
  /** Index into {@link COMPARED} and into every array in `TwinOutput`. */
  index: number;
  /** Flex grow, so the hero cell can be given more height than its context. */
  grow: number;
  /** Cursor sync group, shared by every cell on the screen. */
  syncKey: string;
}

/**
 * Residual y range: symmetric, data-driven, floored.
 *
 * Symmetric because zero is the meaningful centre of a residual and an
 * autoscaled asymmetric axis moves that centre off the middle of the strip,
 * where the eye puts it anyway.
 */
function residualRange(min: number, max: number): [number, number] {
  const peak = Math.max(Math.abs(min), Math.abs(max));
  const half = Number.isFinite(peak) ? Math.max(peak * RESIDUAL_HEADROOM, RESIDUAL_FLOOR) : 4;
  return [-half, half];
}

/** Autoscale the trace pair together, never to zero, with room top and bottom. */
function traceRange(min: number, max: number, minSpan: number): [number, number] {
  if (!Number.isFinite(min) || !Number.isFinite(max)) return [0, minSpan];
  const mid = (min + max) / 2;
  const span = Math.max(max - min, minSpan) / TRACE_FILL;
  return [mid - span / 2, mid + span / 2];
}

/** Three hairlines at the quarters, the same weight the strips use. */
function drawTraceGrid(u: uPlot): void {
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

/** The tolerance region and the zero rule, drawn under the residual trace. */
function drawBand(u: uPlot): void {
  const { ctx } = u;
  const { left, top, width, height } = u.bbox;
  const yTop = u.valToPos(BAND_SIGMA, "y", true);
  const yBottom = u.valToPos(-BAND_SIGMA, "y", true);
  const clampedTop = Math.max(top, yTop);
  const clampedBottom = Math.min(top + height, yBottom);

  ctx.save();
  ctx.fillStyle = token("--muted") || "#0f0f10";
  ctx.fillRect(left, clampedTop, width, Math.max(0, clampedBottom - clampedTop));

  ctx.strokeStyle = token("--border") || "#232326";
  ctx.lineWidth = 1;
  for (const y of [yTop, yBottom]) {
    if (y < top || y > top + height) continue;
    const px = Math.round(y) + 0.5;
    ctx.beginPath();
    ctx.moveTo(left, px);
    ctx.lineTo(left + width, px);
    ctx.stroke();
  }

  // Zero is drawn at schematic weight rather than grid weight, because on a
  // channel carrying a standing offset it is the only thing on the strip the
  // trace can be read against: the band fill covers the whole panel until the
  // residual is large enough to shrink it.
  ctx.strokeStyle = token("--structure") || "#3a3a40";
  const zero = Math.round(u.valToPos(0, "y", true)) + 0.5;
  ctx.beginPath();
  ctx.moveTo(left, zero);
  ctx.lineTo(left + width, zero);
  ctx.stroke();
  ctx.restore();
}

/**
 * Repaint the parts of the residual that leave the band, in the accent.
 *
 * Drawn here rather than as a second uPlot series holding gaps, because a series
 * with holes in it means writing non-finite values into a canvas-backed array,
 * and this app guards those at the store boundary instead. Colour is the alarm
 * channel and this is the one thing on the strip that has earned it.
 */
function drawExcursion(u: uPlot): void {
  const values = u.data[1];
  const times = u.data[0];
  if (!values || !times) return;
  const { ctx } = u;
  const { left, top, width, height } = u.bbox;

  ctx.save();
  ctx.beginPath();
  ctx.rect(left, top, width, height);
  ctx.clip();
  ctx.strokeStyle = token("--primary") || "#ff6b35";
  ctx.fillStyle = token("--primary") || "#ff6b35";
  ctx.lineWidth = 2;

  const zero = u.valToPos(0, "y", true);
  let open = false;
  let startX = 0;
  for (let i = 0; i < values.length; i += 1) {
    const v = values[i];
    const out = v !== null && v !== undefined && Math.abs(v) > BAND_SIGMA;
    const x = u.valToPos(times[i]!, "x", true);
    if (out) {
      const y = u.valToPos(v, "y", true);
      if (!open) {
        open = true;
        startX = x;
        ctx.beginPath();
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    } else if (open) {
      ctx.stroke();
      // The fill closes back along the zero rule, so its area is the residual's
      // own excursion rather than a shape that depends on where the axis ends.
      ctx.lineTo(x, zero);
      ctx.lineTo(startX, zero);
      ctx.closePath();
      ctx.globalAlpha = 0.3;
      ctx.fill();
      ctx.globalAlpha = 1;
      open = false;
    }
  }
  if (open) {
    ctx.stroke();
    const lastX = u.valToPos(times[times.length - 1]!, "x", true);
    ctx.lineTo(lastX, zero);
    ctx.lineTo(startX, zero);
    ctx.closePath();
    ctx.globalAlpha = 0.3;
    ctx.fill();
    ctx.globalAlpha = 1;
  }
  ctx.restore();
}

export function PairedCell({ index, grow, syncKey }: PairedCellProps) {
  const traceHost = useRef<HTMLDivElement>(null);
  const residualHost = useRef<HTMLDivElement>(null);
  const measRef = useRef<HTMLSpanElement>(null);
  const twinRef = useRef<HTMLSpanElement>(null);
  const residRef = useRef<HTMLSpanElement>(null);
  const nameRef = useRef<HTMLSpanElement>(null);
  const staleRef = useRef<HTMLSpanElement>(null);

  const ch = COMPARED[index]!;

  useEffect(() => {
    const traceEl = traceHost.current;
    const residualEl = residualHost.current;
    if (!traceEl || !residualEl) return;

    const common = {
      padding: [4, 0, 2, 0] as uPlot.Padding,
      legend: { show: false },
      axes: [{ show: false }, { show: false }],
      cursor: {
        sync: { key: syncKey, scales: ["x", null] as [string, null] },
        drag: { x: false, y: false },
        points: { show: false },
        y: false,
      },
    };

    const trace = new uPlot(
      {
        ...common,
        width: traceEl.clientWidth || 1,
        height: traceEl.clientHeight || 1,
        scales: {
          x: { time: false },
          y: {
            // The floor is the channel's own tolerance rather than a constant:
            // six sigma of headroom keeps sensor noise from being autoscaled up
            // to fill the panel, and it is the same sigma the residual is
            // normalised by, so a visible gap here is a visible gap there.
            range: (_u, min, max) => {
              const sigma = telemetry.latest?.twin?.sigma[index];
              const span = (Number.isFinite(sigma) ? sigma! : 0) * ch.scale * 6;
              return traceRange(min, max, span > 0 ? span : 1);
            },
          },
        },
        series: [
          {},
          { stroke: token("--measured"), width: 2, points: { show: false } },
          {
            stroke: token("--predicted"),
            width: 1.5,
            dash: [6, 4],
            points: { show: false },
          },
        ],
        hooks: { drawClear: [drawTraceGrid] },
      },
      [new Float64Array(0), new Float64Array(0), new Float64Array(0)] as uPlot.AlignedData,
      traceEl,
    );

    const residual = new uPlot(
      {
        ...common,
        width: residualEl.clientWidth || 1,
        height: residualEl.clientHeight || 1,
        scales: { x: { time: false }, y: { range: (_u, min, max) => residualRange(min, max) } },
        series: [
          {},
          {
            stroke: token("--muted-foreground"),
            width: 1.25,
            fill: "rgb(142 142 150 / 0.14)",
            fillTo: () => 0,
            points: { show: false },
          },
        ],
        hooks: { drawClear: [drawBand], draw: [drawExcursion] },
      },
      [new Float64Array(0), new Float64Array(0)] as uPlot.AlignedData,
      residualEl,
    );

    const resize = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const box = entry.contentRect;
        if (box.width <= 0 || box.height <= 0) continue;
        const chart = entry.target === traceEl ? trace : residual;
        chart.setSize({ width: box.width, height: box.height });
      }
    });
    resize.observe(traceEl);
    resize.observe(residualEl);

    // Redraw only when a frame arrived. The display refreshes three times per
    // telemetry frame and rescaling both charts for unchanged data is wasted.
    let seen = -1;
    const stop = subscribe((frame) => {
      if (frame.seq === seen) return;
      seen = frame.seq;
      const history = telemetry.twin;
      const time = history.time.view();
      const measured = history.measured(index);
      const predicted = history.predicted(index);
      const normalised = history.normalised(index);
      if (!measured || !predicted || !normalised) return;
      trace.setData([time, measured.view(), predicted.view()] as uPlot.AlignedData);
      residual.setData([time, normalised.view()] as uPlot.AlignedData);
    });

    return () => {
      stop();
      resize.disconnect();
      trace.destroy();
      residual.destroy();
    };
  }, [index, syncKey, ch.scale]);

  useLiveSink((frame) => {
    const twin = frame.twin;
    const fresh = isFresh(frame.ages[ch.source]);

    // The plots keep their history when the feed dies, because history is what
    // happened and deleting it would be a lie of a different kind. What they must
    // not do is keep looking current: a frozen trace is indistinguishable from a
    // steady one, so the cell says the word and dims the canvases, which is the
    // same pair `Strip` uses and the same `data-stale` opacity the theme defines.
    if (staleRef.current) staleRef.current.hidden = fresh && twin !== null;
    for (const host of [traceHost.current, residualHost.current]) {
      host?.toggleAttribute("data-stale", !fresh || twin === null);
    }

    const write = (el: HTMLSpanElement | null, text: string, alarm = false) => {
      if (!el) return;
      if (el.textContent !== text) el.textContent = text;
      el.toggleAttribute("data-stale", !fresh);
      el.toggleAttribute("data-alarm", alarm && fresh);
    };

    if (!twin || !fresh) {
      write(measRef.current, NO_VALUE);
      write(twinRef.current, NO_VALUE);
      write(residRef.current, NO_VALUE);
      if (nameRef.current) nameRef.current.toggleAttribute("data-alarm", false);
      return;
    }

    const predicted = (twin.predicted[index] ?? Number.NaN) * ch.scale;
    const measured = predicted + (twin.residual[index] ?? Number.NaN) * ch.scale;
    const sigma = twin.normalised[index] ?? Number.NaN;
    const out = Math.abs(sigma) > BAND_SIGMA;
    const show = (v: number) => (ch.dp === 0 && Math.abs(v) >= 1000 ? grouped(v) : fmt(v, ch.dp));

    write(measRef.current, show(measured), out);
    write(twinRef.current, show(predicted));
    write(residRef.current, `${signed(sigma, 2)}σ`, out);
    if (nameRef.current) nameRef.current.toggleAttribute("data-alarm", out);
  });

  return (
    <div
      className="border-border flex min-h-0 min-w-0 flex-col overflow-hidden border-b"
      style={{ flex: `${grow} 1 0` }}
    >
      <div className="flex h-[30px] shrink-0 items-center justify-between gap-4 px-[18px]">
        <div className="flex min-w-0 items-baseline gap-3">
          <span ref={nameRef} className="t-section data-[alarm]:text-primary whitespace-nowrap">
            {ch.name}
          </span>
          <span className="label-micro truncate">
            {ch.unit || "ratio"}
            <span className="text-structure mx-[6px]">·</span>
            {ch.note}
            <span ref={staleRef} hidden className="text-primary ml-[6px]">
              · STALE
            </span>
          </span>
        </div>
        <div className="flex shrink-0 items-baseline gap-5">
          <span className="label-micro whitespace-nowrap">
            MEAS
            <span
              ref={measRef}
              className="num text-foreground ml-[6px] text-[14px] tracking-normal normal-case"
            >
              {NO_VALUE}
            </span>
          </span>
          <span className="label-micro whitespace-nowrap">
            TWIN
            <span
              ref={twinRef}
              className="num text-predicted ml-[6px] text-[14px] tracking-normal normal-case"
            >
              {NO_VALUE}
            </span>
          </span>
          <span className="label-micro whitespace-nowrap">
            RESID
            <span
              ref={residRef}
              className="num text-foreground ml-[6px] text-[14px] tracking-normal normal-case"
            >
              {NO_VALUE}
            </span>
          </span>
        </div>
      </div>
      <div ref={traceHost} className="min-h-0 min-w-0 flex-[7] px-[18px]" />
      <div
        ref={residualHost}
        className="border-border min-h-0 min-w-0 flex-[3] border-t px-[18px] pb-2"
      />
    </div>
  );
}
