/**
 * What maintenance to raise, selected by the diagnosis.
 *
 * MOCK: the text is templated, from `./advisory`. The selection is live.
 *
 * The artboard's two outline buttons, `SCHEDULE RTB` and `DERATE CYL 3`, are not
 * built. Neither has anything behind it: there is no dispatch path to schedule a
 * return and no per-cylinder derate command on this bus. A control that looks
 * live and does nothing is worse on a demonstration than no control, because the
 * one person certain to press it is the person deciding whether to believe the
 * rest of the screen.
 */

import { useRef } from "react";

import { TASKS } from "@/components/analysis/advisory";
import { NO_VALUE } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import { isFresh } from "@/lib/telemetry";

export function AdvisoryPanel() {
  const action = useRef<HTMLDivElement>(null);
  const duration = useRef<HTMLSpanElement>(null);
  const parts = useRef<HTMLSpanElement>(null);
  const risk = useRef<HTMLSpanElement>(null);

  useLiveSink((frame) => {
    const fresh = isFresh(frame.ages.engine_ms);
    const best = frame.twin?.diagnosis.best ?? 0;
    const task = fresh ? TASKS[best] : null;

    if (action.current) {
      action.current.textContent = task
        ? task.action
        : fresh
          ? "No action · every parameter is at nominal"
          : "Waiting for the twin";
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
      {/* Scrolls rather than clips. `design.md` 10: long content gets
          overflow-y auto, never a silent truncation, and the deferral line is
          the one piece of copy here an operator acts on. */}
      <div className="min-h-0 overflow-y-auto px-4 pt-3 pb-3">
        <div ref={action} className="t-body text-foreground" />
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
