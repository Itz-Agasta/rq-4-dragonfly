/**
 * Where the aircraft was, on the same time base as the track above it.
 *
 * Altitude and outside air temperature rather than altitude alone: the panel had
 * a void under a single trace, and the second profile is what makes the thermal
 * channels on the strips readable, since a cylinder head cooling by 6 K over an
 * hour of descent is not a fault.
 */

import { useMemo, useRef } from "react";

import { fmt, kelvinToCelsius, metresToFeet } from "@/lib/fmt";
import { Live, useLiveSink } from "@/lib/live";
import type { Frame } from "@/lib/telemetry";
import { session, useReplay } from "@/store/replay";

const BOX = 100;
const PAD = 12;
const MAX_POINTS = 400;

/**
 * Range below which a channel is called constant rather than plotted.
 *
 * A generated cruise holds altitude to a tenth of a foot, and scaling a trace to
 * its own extremes turns that into a full-height wander that reads as a climb.
 * A flat line down the middle and the value in the corner is the honest picture.
 */
const FLAT_SPAN = 0.05;

/** Sea level standard temperature, K. ISA, published. */
const ISA_SEA_LEVEL_K = 288.15;

/** Tropospheric lapse rate, K per metre. ISA, published. */
const ISA_LAPSE_K_PER_M = 0.0065;

/** Top of the ISA troposphere, m. Above this the standard atmosphere is isothermal. */
const TROPOPAUSE_M = 11_000;

/** Tropopause temperature, K. ISA, published. */
const TROPOPAUSE_K = 216.65;

/**
 * How far the air is from the standard atmosphere at that altitude.
 *
 * Derived here rather than carried on the wire. Nothing on the bus measures it,
 * and the two quantities it comes from are both in the frame, so a stored copy
 * could only ever disagree with them.
 */
function isaDeviation(frame: Frame): number {
  const standard =
    frame.altitude_m < TROPOPAUSE_M
      ? ISA_SEA_LEVEL_K - ISA_LAPSE_K_PER_M * frame.altitude_m
      : TROPOPAUSE_K;
  return frame.oat_k - standard;
}

export function Profile() {
  const frames = useReplay((s) => s.frames);
  const altitude = useMemo(() => trace(frames, (f) => metresToFeet(f.altitude_m) / 1000), [frames]);
  const air = useMemo(() => trace(frames, (f) => kelvinToCelsius(f.oat_k)), [frames]);

  return (
    <div className="border-border flex w-[260px] shrink-0 flex-col overflow-hidden border-r">
      <div className="border-border flex h-9 shrink-0 items-center border-b px-4">
        <span className="t-section">MISSION PROFILE</span>
      </div>

      <Plot title="pressure alt · kft" series={altitude} dp={1} />
      <Plot title="oat · °C" series={air} dp={0} />

      <div className="shrink-0 px-4 pt-[10px] pb-[14px]">
        <div className="label-micro mb-[6px]">environment at playhead</div>
        <Row
          label="Pressure alt"
          unit="kft"
          select={(f) => fmt(metresToFeet(f.altitude_m) / 1000, 1)}
        />
        <Row label="OAT" unit="°C" select={(f) => fmt(kelvinToCelsius(f.oat_k), 1)} />
        <Row label="ISA deviation" unit="K" select={(f) => fmt(isaDeviation(f), 1)} />
        <Row label="IAS" unit="kt" select={(f) => fmt(f.ias_ms / 0.514_444, 0)} />
      </div>
    </div>
  );
}

function Row({
  label,
  unit,
  select,
}: {
  label: string;
  unit: string;
  select: (frame: Frame) => string;
}) {
  return (
    <div className="border-border flex items-baseline justify-between gap-[10px] border-t py-[5px]">
      <span className="text-muted-foreground text-[11px] leading-[1.4] whitespace-nowrap">
        {label}
      </span>
      <span className="num text-[13px] whitespace-nowrap">
        <Live select={select} />
        <span className="text-muted-foreground ml-1 text-[10px]">{unit}</span>
      </span>
    </div>
  );
}

interface Trace {
  d: string;
  lo: number;
  hi: number;
  /** The channel never moved, so it is labelled rather than scaled. */
  flat: boolean;
}

function Plot({ title, series, dp }: { title: string; series: Trace; dp: number }) {
  const marker = useRef<SVGLineElement>(null);

  useLiveSink(() => {
    const x = ((session.t / (session.duration || 1)) * BOX).toFixed(3);
    marker.current?.setAttribute("x1", x);
    marker.current?.setAttribute("x2", x);
  });

  return (
    <div className="border-border flex min-h-16 flex-1 flex-col border-b px-4 pt-[10px] pb-2">
      <div className="flex items-baseline justify-between gap-[10px]">
        <span className="label-micro">{title}</span>
        <span className="num text-foreground-dim text-[10px]">
          {series.flat
            ? `${fmt(series.lo, dp)} constant`
            : `${fmt(series.lo, dp)} – ${fmt(series.hi, dp)}`}
        </span>
      </div>
      <svg
        width="100%"
        height="100%"
        viewBox={`0 0 ${BOX} ${BOX}`}
        preserveAspectRatio="none"
        className="mt-[7px] block min-h-0 flex-1"
      >
        {[30, 65].map((y) => (
          <line
            key={y}
            x1="0"
            y1={y}
            x2={BOX}
            y2={y}
            stroke="var(--grid)"
            strokeWidth="1"
            vectorEffect="non-scaling-stroke"
          />
        ))}
        <path
          d={series.d}
          fill="none"
          stroke="var(--measured)"
          strokeWidth="2"
          vectorEffect="non-scaling-stroke"
        />
        <line
          ref={marker}
          x1="0"
          y1="0"
          x2="0"
          y2={BOX}
          stroke="var(--structure-hi)"
          strokeWidth="1"
          vectorEffect="non-scaling-stroke"
        />
      </svg>
    </div>
  );
}

/** One channel over the whole mission, scaled to its own extremes. */
function trace(frames: Frame[], get: (frame: Frame) => number): Trace {
  const stride = Math.max(1, Math.ceil(frames.length / MAX_POINTS));
  const values: number[] = [];
  for (let i = 0; i < frames.length; i += stride) values.push(get(frames[i]!));

  let lo = Number.POSITIVE_INFINITY;
  let hi = Number.NEGATIVE_INFINITY;
  for (const value of values) {
    if (!Number.isFinite(value)) continue;
    if (value < lo) lo = value;
    if (value > hi) hi = value;
  }
  if (!Number.isFinite(lo)) return { d: "", lo: Number.NaN, hi: Number.NaN, flat: true };
  const flat = hi - lo < FLAT_SPAN;
  const span = flat ? 1 : hi - lo;

  let d = "";
  let open = false;
  for (let i = 0; i < values.length; i += 1) {
    const value = values[i]!;
    if (!Number.isFinite(value)) {
      open = false;
      continue;
    }
    const x = (i / Math.max(1, values.length - 1)) * BOX;
    const y = flat ? BOX / 2 : BOX - (PAD + ((value - lo) / span) * (BOX - 2 * PAD));
    d += `${open ? "L" : "M"}${x.toFixed(2)} ${y.toFixed(2)} `;
    open = true;
  }
  return { d: d.trim(), lo, hi, flat };
}
