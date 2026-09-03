/**
 * The transport and the mission track.
 *
 * The track spans the recorded length and nothing else. A timeline drawn to a
 * planned endurance puts the playhead in unrecorded time, which is the one thing
 * a scrub bar must never do: an operator dragging past the end of a log has to
 * hit the end of the log.
 */

import { useEffect, useMemo, useRef } from "react";

import { missionClock } from "@/lib/fmt";
import { DEGRADED_BELOW } from "@/lib/health";
import { subscribe } from "@/lib/live";
import type { Frame } from "@/lib/telemetry";
import type { Alert } from "@/store/app";
import { session, SPEEDS, useReplay } from "@/store/replay";

/** Plot coordinate space, mapped to the track with `preserveAspectRatio="none"`. */
const BOX = 100;

/** Headroom above and below the health trace, as a fraction of the box. */
const PAD = 14;

/** Most milestones labelled above the track. */
const MAX_LABELS = 5;

/**
 * Closest two labels may sit, as a percentage of the track.
 *
 * The staggered rows buy one overlap and no more, and residuals arrive in
 * bursts a minute apart. A dropped label is still a tick on the track and still
 * a row in the log.
 */
const MIN_LABEL_GAP = 9;

interface Segments {
  /** Before anything was seen. */
  quiet: string;
  /** A residual outside its band, before the detector committed. */
  watch: string;
  /** After the drift detector latched. */
  alarm: string;
}

export function Timeline() {
  const playing = useReplay((s) => s.playing);
  const speed = useReplay((s) => s.speed);
  const setPlaying = useReplay((s) => s.setPlaying);
  const setSpeed = useReplay((s) => s.setSpeed);
  const events = useReplay((s) => s.events);
  const frames = useReplay((s) => s.frames);
  const health = useReplay((s) => s.health);

  const head = useRef<HTMLSpanElement>(null);
  const handle = useRef<HTMLSpanElement>(null);

  // Full rate rather than the readout rate: this is a graphic, and at 500x the
  // playhead crosses a four hour mission in half a minute.
  useEffect(
    () =>
      subscribe(() => {
        const left = `${((session.t / (session.duration || 1)) * 100).toFixed(3)}%`;
        if (head.current) head.current.style.left = left;
        if (handle.current) handle.current.style.left = left;
      }),
    [],
  );

  const segments = useMemo(
    () => healthPath(frames, health, firstEvent(events)),
    [frames, health, events],
  );
  const hours = useMemo(() => hourTicks(frames.at(-1)?.t_s ?? 0), [frames]);
  const labels = useMemo(() => milestones(events), [events]);

  return (
    <div className="border-border flex h-[90px] min-w-0 shrink-0 items-stretch border-b">
      <div className="border-border flex shrink-0 items-center gap-[14px] border-r px-[18px]">
        <div className="flex items-stretch">
          <Transport label="back one minute" onClick={() => session.step(-1)}>
            <path d="M11 0.5 L4 5.5 L11 10.5 Z" fill="currentColor" />
            <rect x="1" y="0.5" width="1.6" height="10" fill="currentColor" />
          </Transport>
          <Transport
            label={playing ? "pause" : "play"}
            onClick={() => setPlaying(!playing)}
            className="border-x-0"
          >
            {playing ? (
              <>
                <rect x="1" y="0.5" width="3.2" height="10" fill="currentColor" />
                <rect x="6.8" y="0.5" width="3.2" height="10" fill="currentColor" />
              </>
            ) : (
              <path d="M2 0.5 L10 5.5 L2 10.5 Z" fill="currentColor" />
            )}
          </Transport>
          <Transport label="forward one minute" onClick={() => session.step(1)}>
            <path d="M1 0.5 L8 5.5 L1 10.5 Z" fill="currentColor" />
            <rect x="9.4" y="0.5" width="1.6" height="10" fill="currentColor" />
          </Transport>
        </div>

        <div className="flex items-stretch">
          {SPEEDS.map((value) => (
            <button
              key={value}
              type="button"
              onClick={() => setSpeed(value)}
              className={`border-border focus-visible:ring-ring -ml-px flex h-8 min-w-10 items-center justify-center border px-2 text-[11px] tracking-[0.04em] focus-visible:ring-1 focus-visible:outline-none ${
                speed === value
                  ? "bg-foreground text-background"
                  : "text-muted-foreground hover:border-structure-hi"
              }`}
            >
              {value}×
            </button>
          ))}
        </div>
      </div>

      <div className="flex min-w-0 flex-1 flex-col py-[7px] pr-6 pl-5">
        <div className="relative min-w-0 shrink-0 basis-[26px]">
          {labels.map((label, i) => (
            <span
              key={label.id}
              className="absolute text-[10px] leading-[1.2] tracking-[0.07em] whitespace-nowrap"
              style={{
                left: `${label.percent}%`,
                top: i % 2 === 0 ? 0 : 13,
                transform: shift(label.percent),
                color: label.caution ? "var(--primary)" : "var(--structure-hi)",
              }}
            >
              {label.text}
            </span>
          ))}
        </div>

        {/* eslint-disable-next-line jsx-a11y/no-static-element-interactions */}
        <div
          onClick={scrub}
          className="border-border bg-card relative min-h-0 flex-1 cursor-crosshair border"
        >
          <svg
            width="100%"
            height="100%"
            viewBox={`0 0 ${BOX} ${BOX}`}
            preserveAspectRatio="none"
            className="absolute inset-0 block"
          >
            <line
              x1="0"
              y1={BOX / 2}
              x2={BOX}
              y2={BOX / 2}
              stroke="var(--grid)"
              strokeWidth="1"
              vectorEffect="non-scaling-stroke"
            />
            <path
              d={segments.quiet}
              fill="none"
              stroke="var(--measured)"
              strokeWidth="2"
              vectorEffect="non-scaling-stroke"
            />
            <path
              d={segments.watch}
              fill="none"
              stroke="var(--muted-foreground)"
              strokeWidth="2"
              vectorEffect="non-scaling-stroke"
            />
            <path
              d={segments.alarm}
              fill="none"
              stroke="var(--primary)"
              strokeWidth="2"
              vectorEffect="non-scaling-stroke"
            />
          </svg>

          {events.map((event) => (
            <span
              key={event.id}
              className="absolute top-0 bottom-0"
              style={{
                left: `${percent(event.t_s)}%`,
                width: event.severity === "caution" ? 2 : 1,
                background: event.severity === "caution" ? "var(--primary)" : "var(--structure-hi)",
              }}
            />
          ))}

          <span ref={head} className="bg-foreground absolute top-0 bottom-0 left-0 w-px" />
          <span
            ref={handle}
            className="bg-foreground absolute -top-[3px] left-0 size-[7px] -translate-x-[3px]"
          />

          {/* The only accent on the timeline, so it says what it is. Left, where
              the trace begins: at the right it lands on the degraded end of the
              trace, which is the part worth seeing. */}
          <span className="label-micro absolute bottom-[3px] left-[6px] bg-black/70 px-1">
            worst subsystem health
          </span>
        </div>

        <div className="relative min-w-0 shrink-0 basis-[15px]">
          {hours.map((hour) => (
            <span
              key={hour.label}
              className="text-foreground-dim absolute top-[3px] text-[10px] tracking-[0.06em] whitespace-nowrap"
              style={{ left: `${hour.percent}%`, transform: shift(hour.percent) }}
            >
              {hour.label}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

function Transport({
  children,
  label,
  onClick,
  className = "",
}: {
  children: React.ReactNode;
  label: string;
  onClick: () => void;
  className?: string;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className={`border-border hover:border-structure-hi focus-visible:ring-ring flex size-8 items-center justify-center border focus-visible:ring-1 focus-visible:outline-none ${className}`}
    >
      <svg width="12" height="11" viewBox="0 0 12 11" className="text-foreground">
        {children}
      </svg>
    </button>
  );
}

function percent(t_s: number): number {
  return session.duration > 0 ? (t_s / session.duration) * 100 : 0;
}

/** Keep a label inside the track at either end rather than centring it off the edge. */
function shift(at: number): string {
  if (at < 6) return "translateX(0)";
  if (at > 94) return "translateX(-100%)";
  return "translateX(-50%)";
}

/** Mission time of the earliest event, or null. */
function firstEvent(events: Alert[]): number | null {
  let earliest: number | null = null;
  for (const event of events) {
    if (earliest === null || event.t_s < earliest) earliest = event.t_s;
  }
  return earliest;
}

/**
 * The health trace, in three segments.
 *
 * Split where the screen's own claims change rather than at authored times:
 * near-white while nothing has been raised, grey once something has, accent once
 * the worst subsystem is degraded on the same threshold the OPS rail accents a
 * row at. The accent is therefore still the alarm meaning and not a third
 * decoration, and it covers the part of the mission an operator would have been
 * acting on.
 */
function healthPath(frames: Frame[], health: Float64Array, watched: number | null): Segments {
  if (health.length === 0) return { quiet: "", watch: "", alarm: "" };

  let lo = Number.POSITIVE_INFINITY;
  let hi = Number.NEGATIVE_INFINITY;
  for (const value of health) {
    if (!Number.isFinite(value)) continue;
    if (value < lo) lo = value;
    if (value > hi) hi = value;
  }
  if (!Number.isFinite(lo)) return { quiet: "", watch: "", alarm: "" };
  const span = hi - lo || 1;

  const point = (i: number) => {
    const x = (i / Math.max(1, health.length - 1)) * BOX;
    const y = BOX - (PAD + ((health[i]! - lo) / span) * (BOX - 2 * PAD));
    return `${x.toFixed(2)} ${y.toFixed(2)}`;
  };

  const quiet: string[] = [];
  const watch: string[] = [];
  const alarm: string[] = [];
  for (let i = 0; i < health.length; i += 1) {
    if (!Number.isFinite(health[i]!)) continue;
    const t = frames[i]!.t_s;
    // Each segment repeats the sample the previous one ended on, or the trace
    // shows a gap at every handover.
    if (health[i]! < DEGRADED_BELOW) {
      if (alarm.length === 0 && watch.length > 0) alarm.push(watch[watch.length - 1]!);
      alarm.push(point(i));
    } else if (watched !== null && t >= watched) {
      if (watch.length === 0 && quiet.length > 0) watch.push(quiet[quiet.length - 1]!);
      watch.push(point(i));
    } else {
      quiet.push(point(i));
    }
  }
  return { quiet: join(quiet), watch: join(watch), alarm: join(alarm) };
}

function join(points: string[]): string {
  return points.length > 1 ? `M${points.join(" L")}` : "";
}

function scrub(event: React.MouseEvent<HTMLDivElement>): void {
  const box = event.currentTarget.getBoundingClientRect();
  session.seekFraction((event.clientX - box.left) / box.width);
}

/** Whole-hour ticks, plus the end of the recording when it is not on one. */
function hourTicks(duration: number): { label: string; percent: number }[] {
  const out = [{ label: "T+0", percent: 0 }];
  for (let hour = 1; hour * 3600 < duration - 300; hour += 1) {
    out.push({ label: `${hour} h`, percent: ((hour * 3600) / duration) * 100 });
  }
  out.push({ label: missionClock(duration).slice(2, 7), percent: 100 });
  return out;
}

/** The events worth naming above the track, oldest first. */
function milestones(events: Alert[]): {
  id: string;
  text: string;
  percent: number;
  caution: boolean;
}[] {
  const seen = new Set<string>();
  let last = Number.NEGATIVE_INFINITY;
  return events
    .toSorted((a, b) => a.t_s - b.t_s)
    .filter((event) => {
      if (seen.has(event.subsystem)) return false;
      const at = percent(event.t_s);
      if (at - last < MIN_LABEL_GAP) return false;
      seen.add(event.subsystem);
      last = at;
      return true;
    })
    .slice(0, MAX_LABELS)
    .map((event) => ({
      id: event.id,
      text: event.subsystem,
      percent: percent(event.t_s),
      caution: event.severity === "caution",
    }));
}
