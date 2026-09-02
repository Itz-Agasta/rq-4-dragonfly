/**
 * Turning a recorded channel into a path string.
 *
 * Shared by the strips and the mission profile because they were drifting: the
 * two copies had different padding and only one of them broke the path at a
 * missing sample.
 */

import type { Frame } from "@/lib/telemetry";

/** Plot coordinate space. Mapped to a cell with `preserveAspectRatio="none"`. */
export const BOX = 100;

/** Headroom above and below a trace, as a fraction of the box. */
const PAD = 10;

/**
 * Range below which a channel is called constant rather than plotted.
 *
 * A generated cruise holds altitude to a tenth of a foot, and scaling a trace to
 * its own extremes turns that into a full height wander that reads as a climb.
 */
const FLAT_SPAN = 1e-3;

/** One channel's values at drawing resolution, plus the scale they need. */
export interface Series {
  values: number[];
  lo: number;
  hi: number;
  /** The channel never moved, so it is labelled rather than scaled. */
  flat: boolean;
}

/**
 * Read a channel out of the frames, one value per drawn point.
 *
 * `max` is the width of the cell in pixels, near enough. The overview pass
 * carries 2,880 frames and a strip is 400 px wide, so drawing every frame would
 * build a 35 kB path string per series to put seven points on each pixel.
 */
export function series(frames: Frame[], get: (f: Frame) => number, max: number): Series {
  const stride = Math.max(1, Math.ceil(frames.length / max));
  const values: number[] = [];
  let lo = Number.POSITIVE_INFINITY;
  let hi = Number.NEGATIVE_INFINITY;
  for (let i = 0; i < frames.length; i += stride) {
    const value = get(frames[i]!);
    values.push(value);
    if (!Number.isFinite(value)) continue;
    if (value < lo) lo = value;
    if (value > hi) hi = value;
  }
  return { values, lo, hi, flat: !Number.isFinite(lo) || hi - lo < FLAT_SPAN };
}

/** The widest scale covering every series, so they can be drawn against each other. */
export function span(all: Series[]): { lo: number; hi: number; flat: boolean } {
  let lo = Number.POSITIVE_INFINITY;
  let hi = Number.NEGATIVE_INFINITY;
  for (const one of all) {
    if (one.lo < lo) lo = one.lo;
    if (one.hi > hi) hi = one.hi;
  }
  return { lo, hi, flat: !Number.isFinite(lo) || hi - lo < FLAT_SPAN };
}

/**
 * A path across the box, or an empty string when there is nothing to draw.
 *
 * A missing sample breaks the path rather than being interpolated over: a
 * channel the recording does not carry must not be drawn as a straight run
 * between the samples either side of it.
 */
export function path(values: number[], scale: { lo: number; hi: number; flat: boolean }): string {
  if (!Number.isFinite(scale.lo)) return "";
  const range = scale.hi - scale.lo || 1;
  let d = "";
  let open = false;
  for (let i = 0; i < values.length; i += 1) {
    const value = values[i]!;
    if (!Number.isFinite(value)) {
      open = false;
      continue;
    }
    const x = (i / Math.max(1, values.length - 1)) * BOX;
    const y = scale.flat ? BOX / 2 : BOX - (PAD + ((value - scale.lo) / range) * (BOX - 2 * PAD));
    d += `${open ? "L" : "M"}${x.toFixed(2)} ${y.toFixed(2)} `;
    open = true;
  }
  return d.trim();
}
