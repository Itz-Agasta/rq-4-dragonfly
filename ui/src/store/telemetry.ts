/**
 * The live telemetry store.
 *
 * A module singleton, deliberately outside React. Frames arrive at 20 Hz; routing
 * them through React state would reconcile the tree twenty times a second and
 * spend the whole frame budget doing it. Components read from here inside the
 * render loop instead (see `@/lib/live`).
 *
 * This is also where non-finite values stop. Every optional field in the wire
 * format is `NaN` when the controller does not measure it, so guarding at the
 * component would mean guarding in every component, and the one that gets
 * forgotten is the one that kills a canvas.
 */

import { Ring } from "@/lib/ring";
import type { Frame } from "@/lib/telemetry";
import { EventLog } from "@/store/events";
import { CHANNELS, RECORDED } from "@/store/frame";
import { ThetaHistory } from "@/store/theta";
import { TwinHistory } from "@/store/twin";

/** Window held for the streaming strips, seconds. */
export const HISTORY_SECONDS = 90;

/** Publish rate of the core. Sets the ring capacity with the window above. */
const FRAME_RATE_HZ = 20;

const CAPACITY = HISTORY_SECONDS * FRAME_RATE_HZ;

class TelemetryStore {
  /** Frame timestamps, seconds since ingest started. The x axis of every strip. */
  readonly time = new Ring(CAPACITY);

  /** One ring per recorded channel. */
  private readonly rings = new Map<string, Ring>(RECORDED.map((id) => [id, new Ring(CAPACITY)]));

  /**
   * Last value seen for each channel that was finite.
   *
   * A channel that goes non-finite mid-flight holds its last real reading in the
   * ring rather than punching a hole in the trace, and is reported unavailable so
   * the readout can say so. Holding the value keeps the chart drawable; the
   * availability flag is what stops it being read as a measurement.
   */
  private readonly held = new Map<string, number>();

  private readonly unavailable = new Set<string>(RECORDED);

  /**
   * Health parameter estimates at 1 Hz over thirty minutes, for the degradation
   * trajectory. Kept here rather than on ANALYSIS so the window survives
   * navigating away from the screen.
   */
  readonly theta = new ThetaHistory();

  /**
   * Measurement, prediction and residual per compared channel, at the frame
   * rate over the same window as the strips.
   *
   * Kept beside the strips rather than inside the TWIN screen for the reason the
   * theta history is: a screen that allocates its own window starts it again
   * every time it is navigated to, and the one thing an operator does after
   * seeing a divergence on OPS is come here to look at it.
   */
  readonly twin = new TwinHistory(CAPACITY);

  /**
   * Mission events, derived from the frames as they arrive.
   *
   * Fed here rather than from a screen so the log is complete whether or not
   * anyone was looking at OPS when the event happened, which is the only way an
   * alert stack is worth having.
   */
  readonly events = new EventLog();

  /** Most recent frame, or null before the first one arrives. */
  latest: Frame | null = null;

  push(frame: Frame): void {
    this.latest = frame;
    this.theta.push(frame);
    this.twin.push(frame);
    this.events.push(frame);
    this.time.push(Number.isFinite(frame.t_s) ? frame.t_s : 0);

    for (const id of RECORDED) {
      const raw = CHANNELS[id]!.get(frame);
      if (Number.isFinite(raw)) {
        this.held.set(id, raw);
        this.unavailable.delete(id);
        this.rings.get(id)!.push(raw);
      } else {
        this.unavailable.add(id);
        this.rings.get(id)!.push(this.held.get(id) ?? 0);
      }
    }
  }

  /** The ring for a channel. Present for every id in the registry. */
  ring(id: string): Ring {
    const r = this.rings.get(id);
    if (!r) throw new Error(`unknown channel: ${id}`);
    return r;
  }

  /** Whether the channel has ever carried a finite reading, and still does. */
  available(id: string): boolean {
    return !this.unavailable.has(id);
  }
}

/** The one store. */
export const telemetry = new TelemetryStore();
