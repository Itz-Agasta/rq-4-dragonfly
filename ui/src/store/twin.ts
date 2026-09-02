/**
 * History of what the twin predicted and what the engine actually did.
 *
 * The frame carries one instant of both; the TWIN screen is an argument about a
 * stretch of flight, so the browser keeps the stretch. Three rings per compared
 * channel: the measurement, the prediction, and the residual in standard
 * deviations, which is the only one of the three that is comparable between
 * channels and is therefore what the rail ranks and what the residual strip
 * plots.
 *
 * # Measurement is derived, not sent
 *
 * `measured = predicted + residual`, in the channel's own units. That is the
 * definition of the residual rather than an approximation of it, and taking it
 * this way has two properties worth the arithmetic: **torque gets a trace at
 * all**, when nothing on the bus measures it and the core reconstructs it from
 * broadcast load and speed; and a trace cannot disagree with the residual drawn
 * underneath it, because they are the same two numbers.
 *
 * # All twenty-two, not the four on screen
 *
 * The hero cell follows the selection, so any channel can become the one being
 * looked at. Recording only the visible four would mean arriving at a channel and
 * waiting ninety seconds for a plot that the frames to fill were already carrying.
 */

import { Ring } from "@/lib/ring";
import type { Frame } from "@/lib/telemetry";
import { COMPARED } from "@/store/compared";

/**
 * Samples the rail's sort key averages over.
 *
 * Five seconds at the publish rate. Ranking on the instant residual reorders the
 * rail several times a second, because most of twenty-two channels sit within a
 * sigma of each other and noise decides which is which; averaging leaves a rank
 * that only moves on a sustained excursion, which is the thing the rail exists to
 * surface. It is deliberately not a longer window: a fault that has just started
 * must climb the rail while someone is still looking at the screen.
 */
const RANK_SAMPLES = 100;

/**
 * Standard deviations the tolerance band spans, each side of zero.
 *
 * The threshold every panel judges a residual against: the rail's accent edge,
 * the sync table's alarm, the cell header, and the shaded region on the residual
 * strip. It lives with the residual rather than with any one of its readers.
 */
export const BAND_SIGMA = 3;

/** Residual statistics over the retained window, in standard deviations. */
export interface Summary {
  /** Mean residual. The twin's standing bias on this channel, not its noise. */
  mean: number;
  /** Standard deviation of the residual about that mean. */
  sd: number;
  /** Most recent value. */
  now: number;
}

export class TwinHistory {
  /** Sample times, seconds since ingest started. Shared x axis of every cell. */
  time: Ring;

  private measuredRings: Ring[] = [];
  private predictedRings: Ring[] = [];
  private normalisedRings: Ring[] = [];

  /** Mission time of the last sample, to notice the core restarting its clock. */
  private last = Number.NEGATIVE_INFINITY;

  private readonly capacity: number;

  constructor(capacity: number) {
    this.capacity = capacity;
    this.time = new Ring(capacity);
    this.allocate();
  }

  push(frame: Frame): void {
    const twin = frame.twin;
    if (!twin || !Number.isFinite(frame.t_s)) return;

    // A clock that went backwards is a different run of the core. Splicing two
    // runs would draw a step at the seam that neither of them had.
    if (frame.t_s < this.last) this.allocate();

    // The whole frame is dropped rather than the offending channel, because the
    // twenty-two rings share one time ring: skipping a single channel would slide
    // it against the other twenty-one and every cell after it would draw a
    // measurement against a prediction from a different instant.
    for (let i = 0; i < COMPARED.length; i += 1) {
      const p = twin.predicted[i];
      const r = twin.residual[i];
      const n = twin.normalised[i];
      if (!Number.isFinite(p!) || !Number.isFinite(r!) || !Number.isFinite(n!)) return;
    }

    this.last = frame.t_s;
    this.time.push(frame.t_s);
    for (let i = 0; i < COMPARED.length; i += 1) {
      const scale = COMPARED[i]!.scale;
      const p = twin.predicted[i]! * scale;
      this.predictedRings[i]!.push(p);
      this.measuredRings[i]!.push(p + twin.residual[i]! * scale);
      this.normalisedRings[i]!.push(twin.normalised[i]!);
    }
  }

  /** Measurement history for one channel, in display units. */
  measured(i: number): Ring | null {
    return this.measuredRings[i] ?? null;
  }

  /** Prediction history for one channel, in display units. */
  predicted(i: number): Ring | null {
    return this.predictedRings[i] ?? null;
  }

  /** Residual history for one channel, in standard deviations. */
  normalised(i: number): Ring | null {
    return this.normalisedRings[i] ?? null;
  }

  /**
   * Residual mean, spread and current value over the retained window.
   *
   * The mean is the interesting one and it is not zero: model-plant mismatch
   * leaves a standing offset on several channels, measured at -0.69 sigma on
   * torque, and a panel that hid it would be claiming an agreement the twin does
   * not have. Null before the first frame with a twin in it.
   */
  summary(i: number): Summary | null {
    const view = this.normalisedRings[i]?.view();
    if (!view || view.length === 0) return null;
    let sum = 0;
    for (const v of view) sum += v;
    const mean = sum / view.length;
    let square = 0;
    for (const v of view) square += (v - mean) * (v - mean);
    return {
      mean,
      sd: Math.sqrt(square / view.length),
      now: view[view.length - 1]!,
    };
  }

  /**
   * Mean residual over the last {@link RANK_SAMPLES}, signed.
   *
   * Both the rail's sort key, through its magnitude, and the number the rail
   * prints. Those have to be the same quantity: a rail sorted on a five second
   * mean while displaying the instantaneous value puts a channel reading -0.17
   * above one reading +0.40 and looks like a broken sort.
   *
   * Zero for a channel with no history, which sorts it to the bottom rather than
   * to the top, so a rail on a bus with no twin reads as quiet instead of as
   * twenty-two simultaneous faults.
   */
  smoothed(i: number): number {
    const view = this.normalisedRings[i]?.view();
    if (!view || view.length === 0) return 0;
    const from = Math.max(0, view.length - RANK_SAMPLES);
    let sum = 0;
    for (let k = from; k < view.length; k += 1) sum += view[k]!;
    return sum / (view.length - from);
  }

  private allocate(): void {
    this.time = new Ring(this.capacity);
    this.measuredRings = COMPARED.map(() => new Ring(this.capacity));
    this.predictedRings = COMPARED.map(() => new Ring(this.capacity));
    this.normalisedRings = COMPARED.map(() => new Ring(this.capacity));
    this.last = Number.NEGATIVE_INFINITY;
  }
}
