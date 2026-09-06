/**
 * What happened, and what the mission amounted to.
 *
 * Every row here was re-derived from the recorded frames by the same rules that
 * raise an alert on OPS, so a mission flown with nobody watching produces the
 * same log as one that was. A row is a seek: clicking it puts the playhead on
 * the event, which is the only navigation this screen needs.
 */

import { useEffect, useRef, useState } from "react";

import { AIRFRAME, ENGINE, ENGINE_SERIAL } from "@/components/app/TopBar";
import { Field } from "@/components/Field";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { fmt, missionClock, NO_VALUE } from "@/lib/fmt";
import { subscribe } from "@/lib/live";
import {
  bytesLabel,
  dateStamp,
  type MissionInfo,
  OVERVIEW_STRIDE,
  rateHz,
  stamp,
} from "@/lib/mission";
import { type MissionReport, session, useReplay } from "@/store/replay";

/** An event, as both the log rows and the report file need it. */
interface Event {
  t_s: number;
  severity: string;
  subsystem: string;
  message: string;
}

/** One built file, weighed, waiting on the confirmation card. */
interface Built {
  name: string;
  text: string;
  type: string;
  bytes: number;
}

/** Both files the card offers, and what they are about. */
interface Export {
  report: Built;
  csv: Built;
  events: number;
}

export function EventLog() {
  const events = useReplay((s) => s.events);
  const report = useReplay((s) => s.report);
  const info = useReplay((s) => s.info);
  const rows = useRef(new Map<string, HTMLButtonElement>());
  /** The built report the card is asking about, or null when it is closed. */
  const [pending, setPending] = useState<Export | null>(null);
  // Taken from the recording's own frame count rather than assumed, so a
  // recording made at another rate states that rate.
  const basis = info ? (rateHz(info) / OVERVIEW_STRIDE).toFixed(1) : "";

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
        {/* The rate is part of the count. These are re-derived from the overview
            pass, so an excursion no sample lands inside is not in this number. */}
        <span className="label-micro num">
          {events.length}
          {basis ? ` · ${basis} Hz` : ""}
        </span>
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
            onClick={() => setPending(build(events, report, info))}
            className="border-foreground hover:bg-foreground hover:text-background focus-visible:ring-ring w-full border py-[9px] text-[11px] tracking-[0.12em] focus-visible:ring-1 focus-visible:outline-none"
          >
            EXPORT REPORT
          </button>
        </div>

        <ConfirmExport ready={pending} info={info} onClose={() => setPending(null)} />
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
 * The two files, built from what is already on screen.
 *
 * Derived here rather than asked of the daemon, which would be a second
 * implementation of the event rules.
 *
 * **A report has to be filable or it is not a report.** The header block is the
 * identity and the detection summary is the finding; without them this was a
 * list of residuals with no record to attach it to.
 *
 * Built before the card opens, so the size on it is this exact string's.
 */
function build(events: Event[], report: MissionReport, info: MissionInfo | null): Export {
  const ordered = events.toSorted((a, b) => a.t_s - b.t_s);
  const stem = `${AIRFRAME}${info?.recorded_at ? `_${dateStamp(info.recorded_at)}` : ""}`;
  return {
    report: file(`${stem}_report.txt`, "text/plain", reportText(ordered, report, info)),
    csv: file(`${stem}_events.csv`, "text/csv", csvText(ordered)),
    events: events.length,
  };
}

/** A built file and its measured length. */
function file(name: string, type: string, text: string): Built {
  return {
    name,
    text,
    type,
    // Encoded length, not `text.length`. The sigma in every residual message is
    // two bytes, so the two disagree on every report this product produces and
    // the card is the one place the figure has to be the file's.
    bytes: new TextEncoder().encode(text).length,
  };
}

/** Left column width in the header block, so the values line up. */
const PAD = 13;

function reportText(events: Event[], report: MissionReport, info: MissionInfo | null): string {
  const row = (label: string, value: string) => `${label.padEnd(PAD)}${value}`;
  const hz = info ? rateHz(info) : 0;
  return [
    "RQ-4 DRAGONFLY  MISSION HEALTH REPORT",
    "",
    row("AIRFRAME", AIRFRAME),
    row("ENGINE", `${ENGINE}  ${ENGINE_SERIAL}`),
    row("RECORDING", info?.id ?? "unknown"),
    row("RECORDED", info?.recorded_at ? stamp(info.recorded_at) : "no date on the recording"),
    // `.slice(2)` drops the `T+`, which belongs on a mission timestamp and not on
    // an elapsed time. Every stamp below keeps it.
    row(
      "DURATION",
      `${missionClock(session.duration).slice(2)}   engine hours ${fmt(session.duration / 3600, 1)}`,
    ),
    row("SAMPLES", info ? `${info.frames.toLocaleString("en-US")} frames at ${hz} Hz` : "unknown"),
    "",
    "DETECTION",
    // The absence is the finding, and it is the argument this product exists to
    // make: on a coked injector every certificated limit that could see it is an
    // upper bound, so none of them ever trips. Written as an absence here for the
    // same reason the panel above the button writes it as one.
    row(
      "  TWIN",
      report.detected_s === null
        ? "no drift latched"
        : `drift at ${missionClock(report.detected_s)}`,
    ),
    row(
      "  REDLINE",
      report.redline_s === null
        ? "no certified limit tripped"
        : `${report.redline_channel} at ${missionClock(report.redline_s)}`,
    ),
    row("  EVENTS", String(events.length)),
    "",
    `EVENTS  (${events.length}, derived from the recording at ${hz > 0 ? (hz / OVERVIEW_STRIDE).toFixed(1) : "?"} Hz)`,
    "",
    ...events.map(
      (event) =>
        `${missionClock(event.t_s)}  ${event.severity.toUpperCase().padEnd(9)}  ${event.subsystem.padEnd(12)}  ${event.message}`,
    ),
    "",
  ].join("\n");
}

/**
 * The same events for something that consumes them rather than reads them.
 *
 * `t_s` as well as the clock, because a spreadsheet cannot sort `T+03:36:22` and
 * whatever ingests this will want to join on seconds.
 */
function csvText(events: Event[]): string {
  return [
    "t_s,mission_clock,severity,subsystem,message",
    ...events.map((event) =>
      [
        event.t_s.toFixed(3),
        missionClock(event.t_s),
        event.severity,
        event.subsystem,
        event.message,
      ]
        .map(csvCell)
        .join(","),
    ),
    "",
  ].join("\n");
}

/** RFC 4180 quoting. Every message here contains commas. */
function csvCell(value: string): string {
  return /[",\n]/.test(value) ? `"${value.replaceAll('"', '""')}"` : value;
}

/** Hand a built file to the browser. */
function download({ name, text, type }: Built): void {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  URL.revokeObjectURL(url);
}

/**
 * What is about to be saved, before it is.
 *
 * It replaced a caption reading `119 MB · 434,090 frames`, which are the
 * **recording's** figures on a button that saves a 2 kB report. A CSV of the
 * same events was built and removed; one artefact is enough.
 *
 * The copy says what the file is and what it covers, never where it was
 * assembled: that is a fact about this codebase, not about the download.
 */
function ConfirmExport({
  ready,
  info,
  onClose,
}: {
  ready: Export | null;
  info: MissionInfo | null;
  onClose: () => void;
}) {
  if (!ready) return null;

  return (
    <AlertDialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <AlertDialogContent className="w-[420px] gap-0 p-0 sm:max-w-[420px]">
        <AlertDialogHeader className="border-border place-items-start gap-0 border-b px-4 py-[10px] text-left">
          <AlertDialogTitle className="t-section">EXPORT REPORT</AlertDialogTitle>
        </AlertDialogHeader>

        <div className="border-border border-b px-4 py-[10px]">
          <Field label="mission">
            {AIRFRAME}
            {info?.recorded_at ? ` · ${stamp(info.recorded_at)}` : ""}
          </Field>
          <Field label="covers">
            <AlertDialogDescription className="text-inherit">
              {missionClock(session.duration).slice(2)} and {ready.events}{" "}
              {ready.events === 1 ? "event" : "events"}
              {info ? `, from ${info.frames.toLocaleString("en-US")} recorded frames` : ""}
            </AlertDialogDescription>
          </Field>
        </div>

        <div className="px-4 py-[10px]">
          <Field label="file">
            <span className="num break-all">{ready.report.name}</span>
            <span className="text-muted-foreground"> · {bytesLabel(ready.report.bytes)}</span>
          </Field>
        </div>

        <AlertDialogFooter className="border-border gap-2 border-t px-4 py-[10px]">
          <AlertDialogCancel size="sm" variant="ghost">
            CANCEL
          </AlertDialogCancel>
          <AlertDialogAction size="sm" variant="outline" onClick={() => download(ready.report)}>
            SAVE
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
