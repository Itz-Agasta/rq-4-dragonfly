/**
 * Decimated history of the health parameter estimates.
 *
 * The frame carries where each parameter is now; a degradation trajectory needs
 * where it has been, and nothing on the wire carries a series. Serving one would
 * put 1800 floats per parameter on an endpoint that then has to be polled, so the
 * browser keeps its own, the same way it already does for the six strips.
 *
 * Kept in the store rather than in the ANALYSIS screen so leaving the screen and
 * coming back does not restart a thirty-minute window.
 *
 * Sampled at 1 Hz over 1800 samples to match `prognostics::trend`: the chart and
 * the projection drawn on top of it then describe the same stretch of flight.
 */

import { Ring } from "@/lib/ring";
import type { Frame } from "@/lib/telemetry";

/** Seconds between retained samples. Mirrors `trend::DECIMATION_S`. */
const DECIMATION_S = 1;

/** Retained samples per parameter. Thirty minutes, mirroring `trend::WINDOW`. */
export const WINDOW = 1800;

export class ThetaHistory {
  /** Sample times, seconds since ingest started. The x axis of the trajectory. */
  time = new Ring(WINDOW);

  /** One ring per health parameter, allocated when the first frame names them. */
  private rings: Ring[] = [];

  /** Time the next sample is due, so 20 Hz frames decimate to 1 Hz. */
  private due = Number.NEGATIVE_INFINITY;

  /** Time of the last sample, to notice the core restarting its clock. */
  private last = Number.NEGATIVE_INFINITY;

  push(frame: Frame): void {
    const theta = frame.twin?.theta;
    if (!theta || !Number.isFinite(frame.t_s)) return;

    // A shorter clock than the last sample means a different run of the core, and
    // splicing two missions into one trace would draw a decline that never
    // happened. Start again.
    if (theta.length !== this.rings.length || frame.t_s < this.last) this.reset(theta.length);
    if (frame.t_s < this.due) return;

    // Skipped rather than held forward. This is the store boundary where NaN
    // stops, and dropping the sample is the only option that never invents a
    // value: at 1 Hz over thirty minutes a missing sample is invisible, where a
    // held one draws a flat stretch the engine did not have.
    if (!theta.every(Number.isFinite)) return;

    // Snapped to the grid rather than advanced from the last sample, so a dropped
    // frame does not leave every sample after it offset by the gap.
    this.due = Math.floor(frame.t_s / DECIMATION_S) * DECIMATION_S + DECIMATION_S;
    this.last = frame.t_s;
    this.time.push(frame.t_s);
    theta.forEach((v, i) => this.rings[i]!.push(v));
  }

  /** The ring for one parameter, or null before the first frame. */
  ring(i: number): Ring | null {
    return this.rings[i] ?? null;
  }

  private reset(params: number): void {
    this.time = new Ring(WINDOW);
    this.rings = Array.from({ length: params }, () => new Ring(WINDOW));
    this.due = Number.NEGATIVE_INFINITY;
    this.last = Number.NEGATIVE_INFINITY;
  }
}
