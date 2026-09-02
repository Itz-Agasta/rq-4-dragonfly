/**
 * What happened, and what the mission amounted to.
 *
 * Every row here was re-derived from the recorded frames by the same rules that
 * raise an alert on OPS, so a mission flown with nobody watching produces the
 * same log as one that was. A row is a seek: clicking it puts the playhead on
 * the event, which is the only navigation this screen needs.
 */

import { useEffect, useRef } from "react";

import { fmt, missionClock, NO_VALUE } from "@/lib/fmt";
import { subscribe } from "@/lib/live";
import { bytesLabel } from "@/lib/mission";
import { session, useReplay } from "@/store/replay";

export function EventLog() {
  const events = useReplay((s) => s.events);
  const report = useReplay((s) => s.report);
  const info = useReplay((s) => s.info);
  const rows = useRef(new Map<string, HTMLButtonElement>());

  // The active row follows the playhead, which moves at 500x. Toggled as an
  // attribute from the render loop rather than as React state for the reason
  // every other live element here is.
  useEffect(
    () =>
      subscribe(() => {
        let active = "";
        for (const event of events) {
          if (event.t_s <= session.t) {
            active = event.id;
            break;
          }
        }
        for (const [id, element] of rows.current) {
          const on = id === active;
          if (on !== element.hasAttribute("data-active"))
            element.toggleAttribute("data-active", on);
        }
      }),
    [events],
  );

  return (
    <div className="border-border flex w-[300px] shrink-0 flex-col overflow-hidden border-l">
      <div className="border-border flex h-9 shrink-0 items-center justify-between border-b px-4">
        <span className="t-section">MISSION EVENTS</span>
        <span className="label-micro num">{events.length}</span>
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
        {events.length === 0 && (
          <div className="text-muted-foreground px-4 py-3 text-[11px] leading-[1.4]">
            Nothing left its band for long enough to raise an event.
          </div>
        )}
        {events.map((event) => (
          <button
            key={event.id}
            type="button"
            ref={(element) => {
              if (element) rows.current.set(event.id, element);
              else rows.current.delete(event.id);
            }}
            onClick={() => session.seek(event.t_s)}
            className="group border-border data-[active]:bg-card relative flex min-h-10 shrink-0 flex-col justify-center gap-[3px] border-b px-4 py-[6px] text-left"
          >
            <span className="bg-foreground absolute top-0 bottom-0 left-0 hidden w-[2px] group-data-[active]:block" />
            <span className="flex min-w-0 items-center gap-2">
              <span className="num text-muted-foreground text-[11px] whitespace-nowrap">
                {missionClock(event.t_s).slice(2)}
              </span>
              <span
                className={`shrink-0 border px-[5px] py-px text-[9px] tracking-[0.1em] whitespace-nowrap ${
                  event.severity === "caution"
                    ? "border-primary text-primary"
                    : "border-structure text-muted-foreground"
                }`}
              >
                {event.severity === "caution" ? "CAUT" : "ADV"}
              </span>
              <span className="label-micro truncate">{event.subsystem}</span>
            </span>
            <span
              className={`text-[11px] leading-[1.4] text-pretty ${
                event.severity === "caution" ? "text-foreground" : "text-muted-foreground"
              }`}
            >
              {event.message}
            </span>
          </button>
        ))}
      </div>

      <div className="border-border shrink-0 border-t">
        <div className="border-border flex h-8 items-center border-b px-4">
          <span className="text-[12px] tracking-[0.08em]">MISSION HEALTH REPORT</span>
        </div>

        <div className="border-border border-b px-4 pt-[11px] pb-3">
          <div className="label-micro">detection vs certified threshold</div>
          <div className="mt-[5px] flex items-baseline gap-[10px]">
            <span className="t-value num">
              {report.detected_s === null ? NO_VALUE : missionClock(report.detected_s).slice(2)}
            </span>
            <span className="text-muted-foreground text-[11px]">twin</span>
            <span className="text-foreground-dim ml-auto text-[10px] whitespace-nowrap">
              {/* The absence is the argument: on a coked injector every
                  certificated limit that could see it is an upper bound, so
                  none of them ever trips. Reported as an absence, never as a
                  zero lead time. */}
              {report.redline_s === null
                ? "REDLINE no trip"
                : `REDLINE ${missionClock(report.redline_s).slice(2)} · ${report.redline_channel}`}
            </span>
          </div>
        </div>

        <div className="border-border grid grid-cols-3 border-b">
          <Tile label="duration" value={missionClock(session.duration).slice(2)} />
          <Tile label="eng hrs" value={fmt(session.duration / 3600, 1)} />
          <Tile label="events" value={String(report.events)} last />
        </div>

        <div className="px-4 pt-[10px] pb-[14px]">
          <button
            type="button"
            onClick={() => exportReport(events, info?.id ?? "mission")}
            className="border-foreground hover:bg-foreground hover:text-background focus-visible:ring-ring w-full border py-[9px] text-[11px] tracking-[0.12em] focus-visible:ring-1 focus-visible:outline-none"
          >
            EXPORT REPORT
          </button>
          <div className="label-micro mt-2 text-center">
            {info
              ? `${bytesLabel(info.bytes)} · ${info.frames.toLocaleString("en-US")} frames`
              : ""}
          </div>
        </div>
      </div>
    </div>
  );
}

function Tile({ label, value, last = false }: { label: string; value: string; last?: boolean }) {
  return (
    <div className={`px-3 pt-[9px] pb-[10px] ${last ? "" : "border-border border-r"}`}>
      <div className="label-micro">{label}</div>
      <div className="num mt-1 text-[13px]">{value}</div>
    </div>
  );
}

/**
 * The log as a file, built from what is already on screen.
 *
 * Not a server-side report: every line of it was derived here from the recorded
 * frames, and asking the daemon for it would be a second implementation of the
 * event rules.
 */
function exportReport(
  events: { t_s: number; severity: string; subsystem: string; message: string }[],
  id: string,
): void {
  const lines = [
    `RQ-4 DRAGONFLY mission report`,
    `mission     ${id}`,
    `duration    ${missionClock(session.duration)}`,
    `events      ${events.length}`,
    "",
    ...events
      .toSorted((a, b) => a.t_s - b.t_s)
      .map(
        (event) =>
          `${missionClock(event.t_s)}  ${event.severity.toUpperCase().padEnd(9)}  ${event.subsystem.padEnd(12)}  ${event.message}`,
      ),
  ];
  const url = URL.createObjectURL(new Blob([lines.join("\n")], { type: "text/plain" }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = `${id}-report.txt`;
  anchor.click();
  URL.revokeObjectURL(url);
}
