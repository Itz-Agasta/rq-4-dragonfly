/**
 * Turning a channel into a path string.
 *
 * Shared by REPLAY's strips and mission profile because they were drifting: the
 * two copies had different padding and only one of them broke the path at a
 * missing sample. SIMULATE's projected channels arrive as plain arrays rather
 * than frames, which is why [`scaleOf`] exists beside [`series`].
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

/**
 * Span below which a projected channel is called constant, as a fraction of its
 * own magnitude.
 *
 * Three parts in a thousand, which is loose, and it can be because a projection
 * **freezes the health estimate at the seed**. Nothing wears over the horizon, so
 * there is no slow degradation trend for this to hide: a sub-percent drift on a
 * steady leg is the boost controller and the governor converging, measured at a
 * ten second time constant and gone by the seventh sample. On a profile that does
 * something the spans are one to ten percent and nowhere near this.
 */
const RELATIVE_FLAT = 3e-3;

/** What a trace is drawn against: an extent, and whether it is worth scaling to. */
export interface Scale {
  lo: number;
  hi: number;
  /** The channel never moved, so it is labelled rather than scaled. */
  flat: boolean;
}

/** One channel's values at drawing resolution, plus the scale they need. */
export interface Series extends Scale {
  values: number[];
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
export function span(all: Series[]): Scale {
  let lo = Number.POSITIVE_INFINITY;
  let hi = Number.NEGATIVE_INFINITY;
  for (const one of all) {
    if (one.lo < lo) lo = one.lo;
    if (one.hi > hi) hi = one.hi;
  }
  return { lo, hi, flat: !Number.isFinite(lo) || hi - lo < FLAT_SPAN };
}

/**
 * Where one value sits vertically in the box.
 *
 * Exported so a limit line and the trace it judges are placed by the same
 * arithmetic. A limit drawn by a second calculation is a limit that can sit a
 * pixel off the crossing it is supposed to explain.
 */
export function yOf(value: number, scale: Scale): number {
  if (scale.flat) return BOX / 2;
  const range = scale.hi - scale.lo || 1;
  return BOX - (PAD + ((value - scale.lo) / range) * (BOX - 2 * PAD));
}

/**
 * A path across the box, or an empty string when there is nothing to draw.
 *
 * A missing sample breaks the path rather than being interpolated over: a
 * channel the recording does not carry must not be drawn as a straight run
 * between the samples either side of it.
 */
export function path(values: number[], scale: Scale): string {
  if (!Number.isFinite(scale.lo)) return "";
  let d = "";
  let open = false;
  for (let i = 0; i < values.length; i += 1) {
    const value = values[i]!;
    if (!Number.isFinite(value)) {
      open = false;
      continue;
    }
    const x = (i / Math.max(1, values.length - 1)) * BOX;
    const y = yOf(value, scale);
    d += `${open ? "L" : "M"}${x.toFixed(2)} ${y.toFixed(2)} `;
    open = true;
  }
  return d.trim();
}

/**
 * The scale an already-sampled channel needs.
 *
 * A projection arrives at a fixed sample count with no frames behind it, so
 * there is nothing to decimate and nothing to read out of; only the range is
 * missing. `floor` pulls a limit line into view when the trace sits under it,
 * so a channel drawn against a limit shows the headroom rather than filling the
 * cell and implying there is none.
 *
 * Flatness is relative here where [`series`] takes it absolutely, because a
 * projected channel is read in its own engineering unit against a limit in the
 * same unit. Fuel flow wandering 0.002 kg/h around 11 is a fifth of a tenth of
 * a percent, and scaling that to the cell draws a step change in fuelling that
 * did not happen. The absolute test is kept for recordings, where the channels
 * are already normalised by the strip that draws them.
 */
export function scaleOf(values: number[], floor?: number): Series {
  let lo = Number.POSITIVE_INFINITY;
  let hi = Number.NEGATIVE_INFINITY;
  for (const value of values) {
    if (!Number.isFinite(value)) continue;
    if (value < lo) lo = value;
    if (value > hi) hi = value;
  }
  const flat = !Number.isFinite(lo) || hi - lo < Math.max(FLAT_SPAN, Math.abs(hi) * RELATIVE_FLAT);
  if (floor !== undefined && Number.isFinite(lo)) hi = Math.max(hi, floor);
  return { values, lo, hi, flat };
}
