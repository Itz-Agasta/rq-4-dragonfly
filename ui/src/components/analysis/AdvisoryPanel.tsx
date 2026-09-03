/**
 * What maintenance to raise, selected by the diagnosis.
 *
 * MOCK: the text is templated, from `./advisory`. The selection is live.
 *
 * `SCHEDULE RTB` and `DERATE CYL 3` controls are deliberately not built. Neither
 * has anything behind it: there is no dispatch path to schedule a return and no
 * per-cylinder derate command on this bus. A control that looks
 * live and does nothing is worse on a demonstration than no control, because the
 * one person certain to press it is the person deciding whether to believe the
 * rest of the screen.
 */

import { useRef } from "react";

import { degradedNote, TASKS } from "@/lib/advisory";
import { NO_VALUE } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import { isFresh } from "@/lib/telemetry";

/** Shown before a frame arrives and whenever the twin has no estimate. */
const WAITING = "Waiting for the twin";

export function AdvisoryPanel() {
  const action = useRef<HTMLDivElement>(null);
  const duration = useRef<HTMLSpanElement>(null);
  const parts = useRef<HTMLSpanElement>(null);
  const risk = useRef<HTMLSpanElement>(null);

  useLiveSink((frame) => {
    const twin = frame.twin;
    // No twin is not a diagnosis of NOMINAL. Defaulting the hypothesis index to
    // zero reads an absent estimate as the nominal row, and the panel then clears
    // an engine no estimator has looked at yet.
    const diagnosed = twin !== null && isFresh(frame.ages.engine_ms);
    // See `ops/Advisory`: a ranking that does not fit must not become a repair.
    const task = diagnosed && !twin.diagnosis.unexplained ? TASKS[twin.diagnosis.best] : null;

    if (action.current) {
      const degraded = diagnosed ? degradedNote(twin.health, twin.ever_locked) : null;
      action.current.textContent = task
        ? task.action
        : diagnosed
          ? (degraded ?? "No action · every parameter is at nominal")
          : WAITING;
    }
    if (duration.current) duration.current.textContent = task?.duration ?? NO_VALUE;
    if (parts.current) parts.current.textContent = task?.parts ?? NO_VALUE;
    if (risk.current) risk.current.textContent = task?.risk ?? "";
  });

  return (
    <div className="flex min-h-0 shrink flex-col">
      <div className="border-border flex h-8 shrink-0 items-center justify-between border-t border-b px-4">
        <span className="text-[12px] tracking-[0.08em]">MAINTENANCE ADVISORY</span>
        <span className="border-structure-hi text-foreground-dim border border-dashed px-[6px] py-[2px] text-[10px] tracking-[0.08em]">
          ◊ INFERRED
        </span>
      </div>
      {/* Scrolls rather than clips: the deferral line is the one piece of copy
          here an operator acts on, so it must never truncate silently. */}
      <div className="min-h-0 overflow-y-auto px-4 pt-3 pb-3">
        {/* Reads before the first frame as it does without a twin, so the panel
            never starts blank and never starts clear. */}
        <div ref={action} className="t-body text-foreground">
          {WAITING}
        </div>
        <dl className="mt-2 grid grid-cols-[46px_minmax(0,1fr)] gap-x-[10px] gap-y-1">
          <dt className="label-micro">dur</dt>
          <dd>
            <span ref={duration} className="t-small text-foreground" />
          </dd>
          <dt className="label-micro">parts</dt>
          <dd className="min-w-0">
            <span ref={parts} className="t-small text-foreground block truncate" />
          </dd>
          <dt className="label-micro">risk</dt>
          <dd>
            <span ref={risk} className="t-small text-muted-foreground block text-pretty" />
          </dd>
        </dl>
      </div>
    </div>
  );
}
