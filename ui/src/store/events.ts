/**
 * Mission events, derived from the frame rather than sent on the wire.
 *
 * Every input is already in the frame: what the detectors made of it, which
 * channel carried the excursion, whether the certified redline tripped, and all
 * twenty-two normalised residuals. Deriving here rather than raising events in
 * the core keeps one implementation for two jobs, because D12's replay feeds
 * recorded frames through this same code and gets the same events with the same
 * ids and the same mission times. An event raised in the core would have to be
 * carried on the wire, held for some window, and then reconciled against
 * whatever the client re-derived.
 *
 * # The log starts when the client connects
 *
 * A residual excursion that began before the page was opened is timestamped from
 * when this log first saw it, because the client has no history it did not
 * receive. The two events that come from the core's own detectors, the CUSUM
 * drift and the redline trip, carry the mission time the core latched them at and
 * survive a reload intact. D12's recorder closes the gap properly by feeding
 * recorded frames through this same code from the start of the mission.
 *
 * # Nothing here names a subsystem
 *
 * An event says what was observed and which channel observed it, never which
 * part of the engine is at fault. That is the D9 finding: a residual excursion
 * on EGT 3 is consistent with a coked injector, a misfire and a lying
 * thermocouple, and the rail reports what the filter spent rather than what
 * happened. Naming the subsystem is the diagnosis layer's job and the advisory
 * panel is where it appears.
 */

import { fmt } from "@/lib/fmt";
import { isFresh, type Frame } from "@/lib/telemetry";
import type { Alert, Severity } from "@/store/app";
import { COMPARED } from "@/store/compared";
import { BAND_SIGMA } from "@/store/twin";

/**
 * Seconds a residual must stay outside the band before it is an event.
 *
 * The band is crossed by noise several times an hour on twenty-two channels, and
 * an alert stack that fills with single-frame crossings is one an operator stops
 * reading. Two seconds is forty frames, which noise does not sustain and a real
 * excursion holds indefinitely.
 */
const SUSTAIN_S = 2;

/**
 * Where a residual has to fall back to before its episode can be considered over.
 *
 * Below the raise threshold on purpose. A residual sitting exactly on three
 * sigma would otherwise open and close an episode continuously, and each cycle
 * would raise a new event with a new id.
 */
const CLEAR_SIGMA = 2.5;

/**
 * Seconds a residual must stay back inside `CLEAR_SIGMA` before its episode ends.
 *
 * A 0.5 sigma dead band alone is not hysteresis at unit-variance noise: a
 * deepening ramp dips back inside for a few frames and raises the same channel
 * twice for one excursion. Ten seconds because the noise is white at the publish
 * rate, so a dip is sub-second, while a channel that has come back stays back.
 */
const CLEAR_S = 10;

/** One channel's excursion state between frames. */
interface Episode {
  /** Mission time the residual first left the band, or null when inside it. */
  since: number | null;
  /** Whether an event has already been raised for this episode. */
  raised: boolean;
  /** Mission time it fell back inside `CLEAR_SIGMA`, or null while outside it. */
  returned: number | null;
}

export class EventLog {
  private alerts: Alert[] = [];
  private episodes = new Map<number, Episode>();
  private linkWasOk = true;
  private wasLocked: boolean | null = null;
  private lastT = Number.NEGATIVE_INFINITY;

  /**
   * Bumped whenever the list changes.
   *
   * A component watches this rather than the array, because the array is rebuilt
   * only when something is actually raised and comparing a counter is what keeps
   * a 20 Hz feed from re-rendering a React list twenty times a second.
   */
  version = 0;

  push(frame: Frame): void {
    if (!Number.isFinite(frame.t_s)) return;

    // A clock that went backwards is a different run. Events from the previous
    // one would sit in the stack with mission times this run never reaches.
    if (frame.t_s < this.lastT) this.reset();
    this.lastT = frame.t_s;

    this.link(frame);
    this.redline(frame);
    this.drift(frame);
    this.lock(frame);
    this.residuals(frame);
  }

  /** Newest first, which is the order the stack reads. */
  list(): Alert[] {
    return this.alerts;
  }

  private raise(alert: Alert): void {
    if (this.alerts.some((a) => a.id === alert.id)) return;
    this.alerts = [alert, ...this.alerts];
    this.version += 1;
  }

  private reset(): void {
    this.alerts = [];
    this.episodes.clear();
    this.linkWasOk = true;
    this.wasLocked = null;
    this.version += 1;
  }

  /** The engine controller falling silent, which invalidates everything else. */
  private link(frame: Frame): void {
    if (!frame.link_ok && this.linkWasOk) {
      this.raise(
        event(frame.t_s, "caution", "LINK", "Engine controller silent, telemetry stale", "ops"),
      );
    }
    this.linkWasOk = frame.link_ok;
  }

  /**
   * The conventional monitor tripping.
   *
   * On the demonstration fault this never fires, and that absence is the whole
   * argument: a coked injector runs its cylinder cooler and every certificated
   * limit that could see it is an upper bound. The rule exists so that the
   * absence is a measurement rather than a missing feature.
   */
  private redline(frame: Frame): void {
    const d = frame.twin?.detection;
    if (!d || d.redline_since === null) return;
    this.raise(
      event(
        d.redline_since,
        "caution",
        "REDLINE",
        `${d.redline_channel} exceeded its certified limit`,
        "analysis",
      ),
    );
  }

  /** The CUSUM, which is the detector that catches a slow degradation. */
  private drift(frame: Frame): void {
    const d = frame.twin?.detection;
    if (!d || !d.drift || d.drift_since === null) return;
    this.raise(
      event(
        d.drift_since,
        "caution",
        d.cusum_channel || "DETECTOR",
        // The accumulated CUSUM is not printed. It grows without bound while a
        // fault is present, so an hour into a coked injector it reads 545,914
        // against a decision interval of 5, which is a true number that tells an
        // operator nothing and looks like a units error.
        `Sustained drift on ${d.cusum_channel}, past its ${fmt(d.cusum_limit, 1)} decision interval`,
        "analysis",
      ),
    );
  }

  /**
   * The twin losing its estimate, which is not an engine fault but is a caveat.
   *
   * An edge, like the link rule, and it has to be: an id is its source and its
   * rounded mission time, so a latch that never cleared raises a fresh alert every
   * second of an outage. A stale frame updates nothing, because its twin is absent
   * rather than unlocked and clearing the latch there loses the real transition.
   */
  private lock(frame: Frame): void {
    if (!isFresh(frame.ages.engine_ms)) return;
    const locked = frame.twin?.locked === true;
    // Seeded from the twin's latch, not `false`: a screen opened mid-outage never
    // saw the edge, so it would raise no caveat and `residuals` would stay
    // suppressed. A cold start has never locked, so it still raises nothing.
    const wasLocked = this.wasLocked ?? frame.twin?.ever_locked === true;
    if (wasLocked && !locked) {
      this.raise(
        event(frame.t_s, "advisory", "TWIN", "Twin lost lock, residuals unreliable", "twin"),
      );
    }
    this.wasLocked = locked;
  }

  /** Channels sitting outside their own tolerance band for long enough to mean it. */
  private residuals(frame: Frame): void {
    const twin = frame.twin;
    if (!twin || !twin.locked || !isFresh(frame.ages.engine_ms)) return;

    for (let i = 0; i < COMPARED.length; i += 1) {
      const sigma = twin.normalised[i];
      if (sigma === undefined || !Number.isFinite(sigma)) continue;
      const magnitude = Math.abs(sigma);
      const episode = this.episodes.get(i) ?? { since: null, raised: false, returned: null };

      // Restarted anywhere at or above the threshold, the dead band included: a
      // residual that fell back and settled at 2.7 sigma has not come back at all.
      if (magnitude >= CLEAR_SIGMA) episode.returned = null;

      if (magnitude > BAND_SIGMA) {
        if (episode.since === null) episode.since = frame.t_s;
        if (!episode.raised && frame.t_s - episode.since >= SUSTAIN_S) {
          episode.raised = true;
          const name = COMPARED[i]!.name;
          this.raise(
            event(
              episode.since,
              "advisory",
              name,
              `${name} residual ${sigma > 0 ? "+" : "−"}${fmt(magnitude, 2)}σ, outside ±${BAND_SIGMA}σ`,
              "twin",
            ),
          );
        }
      } else if (magnitude < CLEAR_SIGMA) {
        if (episode.returned === null) episode.returned = frame.t_s;
        if (frame.t_s - episode.returned >= CLEAR_S) {
          episode.since = null;
          episode.raised = false;
          episode.returned = null;
        }
      }
      this.episodes.set(i, episode);
    }
  }
}

/**
 * One event, with an id derived from what raised it and when.
 *
 * Deterministic on purpose: replaying a recorded mission re-derives the same id
 * for the same event, so an acknowledgement made live still applies.
 */
function event(
  t_s: number,
  severity: Severity,
  source: string,
  message: string,
  screen: Alert["screen"],
): Alert {
  return {
    id: `${source}:${Math.round(t_s)}`,
    t_s,
    severity,
    subsystem: source,
    message,
    screen,
  };
}
