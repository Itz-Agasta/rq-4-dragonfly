/**
 * A loaded recording, and the playhead over it.
 *
 * The frames and everything derived from them are cold, set once when a mission
 * opens; the playhead is hot and moves at 500x, so it stays out of React. One
 * array, referenced by both.
 *
 * The events come from `store/events.ts` run over the recorded frames, so there
 * is one implementation of the rules rather than two. A recorded frame carries
 * zero source ages, which read as fresh, so the freshness gates inside those
 * rules pass rather than suppressing every event.
 *
 * **They are derived at the overview sampling, not at 20 Hz.** The redline and
 * drift rules are exact regardless, because both read a mission time the core
 * latched and put in every frame after it. The residual, link and lock rules
 * stamp `frame.t_s`, so they quantise to the sample period and cannot see an
 * excursion that no sample lands inside. The panel states the basis.
 */

import { create } from "zustand";

import { type MissionInfo, OVERVIEW_STRIDE, readMission } from "@/lib/mission";
import type { Frame } from "@/lib/telemetry";
import type { Alert } from "@/store/app";
import { EventLog } from "@/store/events";

/** Transport speeds, mission seconds per wall second. */
export const SPEEDS = [0.5, 1, 10, 100, 500] as const;

/** Seconds of mission a step button moves the playhead. */
const STEP_S = 60;

/**
 * What the timeline draws inside the track.
 *
 * The lowest of the seven subsystem indices rather than a health parameter: a
 * parameter is in the units of whatever it parameterises and needs a caption to
 * mean anything, while the worst index is on one 0 to 100 scale that reads the
 * same for a coked injector and a fouled radiator. It is the only accent on the
 * timeline, so it carries a label saying what it is.
 *
 * `NaN` until the twin locks. The indices read zero before the lock, which is
 * the absence of a score rather than a score of zero, and plotting it puts a
 * full-scale spike at T+0: a 99 to 33.7 decline over four hours then renders as
 * a flat line.
 */
function worstIndex(frame: Frame): number {
  const health = frame.twin?.health;
  if (!frame.twin?.locked || !health || health.length === 0) return Number.NaN;
  let worst = Number.POSITIVE_INFINITY;
  for (const value of health) {
    if (Number.isFinite(value) && value < worst) worst = value;
  }
  return Number.isFinite(worst) ? worst : Number.NaN;
}

/** What the mission health report says, all of it derived from the frames. */
export interface MissionReport {
  /** Mission time the twin's drift detector latched, or null. */
  detected_s: number | null;
  /** Mission time a certificated limit tripped, or null when none ever did. */
  redline_s: number | null;
  /** Which limit that was. */
  redline_channel: string;
  /** Events raised over the mission. */
  events: number;
}

/**
 * The playhead.
 *
 * Reads the frames out of the store rather than holding them, so there is one
 * array and a component's memo can depend on the same reference this does.
 */
class Playhead {
  /** Mission time, seconds. */
  t = 0;

  /** Mission time of the last frame in the recording. */
  duration = 0;

  private index = 0;
  private handle = 0;
  private last = 0;

  /** The frame under the playhead, or null before a mission is loaded. */
  frame(): Frame | null {
    return useReplay.getState().frames[this.index] ?? null;
  }

  reset(duration: number): void {
    this.stop();
    this.duration = duration;
    this.t = 0;
    this.index = 0;
  }

  /** Move to a mission time, clamped to the recording. */
  seek(t: number): void {
    this.t = Math.max(0, Math.min(this.duration, t));
    this.index = indexAt(useReplay.getState().frames, this.t);
  }

  /** Move by a fraction of the track, which is what a click on it gives. */
  seekFraction(fraction: number): void {
    this.seek(fraction * this.duration);
  }

  step(direction: -1 | 1): void {
    this.seek(this.t + direction * STEP_S);
  }

  /**
   * Advance in wall time until the recording runs out.
   *
   * Driven by its own `requestAnimationFrame` rather than by the shared render
   * loop in `lib/live`, because that loop's job is to read the current frame and
   * this one's is to decide which frame is current. Running the second inside the
   * first would make every readout on screen depend on the order sinks were
   * registered in.
   */
  play(speed: number, onEnd: () => void): void {
    this.stop();
    this.last = performance.now();
    const tick = (now: number) => {
      const dt = (now - this.last) / 1000;
      this.last = now;
      this.seek(this.t + dt * speed);
      if (this.t >= this.duration) {
        this.stop();
        onEnd();
        return;
      }
      this.handle = requestAnimationFrame(tick);
    };
    this.handle = requestAnimationFrame(tick);
  }

  stop(): void {
    if (this.handle !== 0) {
      cancelAnimationFrame(this.handle);
      this.handle = 0;
    }
  }
}

/** Frame at or before a mission time. */
function indexAt(frames: Frame[], t: number): number {
  if (frames.length === 0) return 0;
  let lo = 0;
  let hi = frames.length - 1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (frames[mid]!.t_s <= t) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/** The one playhead. */
export const session = new Playhead();

type Status = "idle" | "loading" | "ready" | "error";

interface ReplayState {
  status: Status;
  /** What went wrong, for the screen to show instead of an empty timeline. */
  error: string;
  info: MissionInfo | null;
  /** The overview pass, oldest first. One reference, replaced on load. */
  frames: Frame[];
  /** Worst subsystem index per frame, aligned with {@link ReplayState.frames}. */
  health: Float64Array;
  /** Newest first, which is the order the log reads. */
  events: Alert[];
  report: MissionReport;
  playing: boolean;
  speed: number;

  open: (info: MissionInfo) => Promise<void>;
  setPlaying: (playing: boolean) => void;
  setSpeed: (speed: number) => void;
}

const NO_REPORT: MissionReport = {
  detected_s: null,
  redline_s: null,
  redline_channel: "",
  events: 0,
};

export const useReplay = create<ReplayState>((set, get) => ({
  status: "idle",
  error: "",
  info: null,
  frames: [],
  health: new Float64Array(0),
  events: [],
  report: NO_REPORT,
  playing: false,
  speed: 10,

  open: async (info) => {
    session.reset(0);
    set({
      status: "loading",
      error: "",
      info,
      playing: false,
      frames: [],
      health: new Float64Array(0),
      events: [],
      report: NO_REPORT,
    });
    try {
      const frames = await readMission(info.id, { stride: OVERVIEW_STRIDE, count: 20_000 });
      const log = new EventLog();
      for (const frame of frames) log.push(frame);
      const events = log.list();
      set({
        status: "ready",
        frames,
        health: new Float64Array(frames.map(worstIndex)),
        events,
        report: summarise(frames, events),
      });
      session.reset(info.duration_s);
    } catch (error) {
      set({ status: "error", error: error instanceof Error ? error.message : String(error) });
    }
  },

  setPlaying: (playing) => {
    if (playing) {
      // Restarting from the end rather than sitting there: the transport is the
      // only way back to the beginning and a play button that does nothing reads
      // as broken.
      if (session.t >= session.duration) session.seek(0);
      session.play(get().speed, () => set({ playing: false }));
    } else {
      session.stop();
    }
    set({ playing });
  },

  setSpeed: (speed) => {
    set({ speed });
    if (get().playing) session.play(speed, () => set({ playing: false }));
  },
}));

/**
 * What the mission amounted to.
 *
 * The lead time is the interval between the twin latching its drift alarm and a
 * certificated limit tripping, and on the demonstration fault the second of
 * those never happens: a coked injector runs its own cylinder cooler and every
 * limit that could see it is an upper bound. The absence is reported as an
 * absence rather than as a zero, because it is the argument the screen exists to
 * make.
 */
function summarise(frames: Frame[], events: Alert[]): MissionReport {
  let detected: number | null = null;
  let redline: number | null = null;
  let channel = "";
  for (const frame of frames) {
    const detection = frame.twin?.detection;
    if (!detection) continue;
    if (detected === null && detection.drift_since !== null) detected = detection.drift_since;
    if (redline === null && detection.redline_since !== null) {
      redline = detection.redline_since;
      channel = detection.redline_channel;
    }
  }
  return {
    detected_s: detected,
    redline_s: redline,
    redline_channel: channel,
    events: events.length,
  };
}
