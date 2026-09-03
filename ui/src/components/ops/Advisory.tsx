/**
 * The AI advisory panel: what the twin thinks is wrong, and what to do.
 *
 * Live, and reading the same diagnosis and prognosis ANALYSIS reads. Two screens
 * describing one engine have to agree, and while this panel was authored they did
 * not: it claimed injector coking at 91% with 6.2 hours left beside an ANALYSIS
 * screen measuring 96.5% and a remaining life the fit will not support.
 *
 * Provenance is typographic and never chromatic: a dashed hairline and a
 * `◊ INFERRED` tag, no colour on the panel at all. The accent is spent on alarm.
 *
 * # The continue-mission risk percentage is gone
 *
 * It read 34% and nothing computes it. A probability with no estimator behind it
 * is the one number on this screen a judge would ask the derivation of, and there
 * is no answer. In its place is the comparison that **is** measured and is the
 * strongest claim the product makes: when the twin raised the fault, against
 * whether the certified redline ever tripped at all.
 */

import { useRef } from "react";

import { degradedNote, TASKS } from "@/lib/advisory";
import { fmt, missionClock, NO_VALUE, posteriorPct } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import { HYPOTHESES, isFresh, SUBSYSTEMS } from "@/lib/telemetry";
const WAITING = "waiting for the twin";

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="min-w-0">
      <div className="label-micro">{label}</div>
      <div className="mt-[5px]">{children}</div>
    </div>
  );
}

function Divider() {
  return <div className="bg-border h-px shrink-0" aria-hidden="true" />;
}

export function Advisory() {
  const diagnosis = useRef<HTMLSpanElement>(null);
  const confidence = useRef<HTMLSpanElement>(null);
  const life = useRef<HTMLSpanElement>(null);
  const lifeNote = useRef<HTMLSpanElement>(null);
  const action = useRef<HTMLSpanElement>(null);
  const detected = useRef<HTMLSpanElement>(null);
  const redline = useRef<HTMLSpanElement>(null);

  useLiveSink((frame) => {
    const twin = frame.twin;
    // An absent twin is not a diagnosis of NOMINAL. Index 0 is the nominal row,
    // so defaulting it clears an engine no estimator has looked at yet.
    const diagnosed = twin !== null && isFresh(frame.ages.engine_ms);

    if (diagnosis.current) {
      diagnosis.current.textContent = !diagnosed
        ? WAITING
        : twin.diagnosis.unexplained
          ? "NO LIBRARY MATCH"
          : (HYPOTHESES[twin.diagnosis.best] ?? NO_VALUE);
    }
    if (confidence.current) {
      const posterior =
        diagnosed && !twin.diagnosis.unexplained
          ? twin.diagnosis.posterior[twin.diagnosis.best]
          : undefined;
      confidence.current.textContent =
        posterior === undefined
          ? diagnosed && twin.diagnosis.unexplained
            ? "no signature fits this residual"
            : ""
          : `confidence ${posteriorPct(posterior, twin?.diagnosis.best === 0)}%`;
    }

    // The limiting subsystem rather than the worst parameter: an operator is
    // deciding whether to keep flying, and what limits the aircraft is whichever
    // subsystem runs out first.
    const limiting = frame.prognosis?.limiting ?? null;
    const rul = limiting === null ? undefined : frame.prognosis?.subsystem[limiting];
    // `hours: null` means nothing is degrading and is **not** zero. Rendering it
    // as 0 h grounds a serviceable aircraft, so it stays a dash and the note
    // beside it says why the dash is there.
    const hours = rul?.hours ?? null;
    const subsystem = limiting === null ? "" : (SUBSYSTEMS[limiting] ?? "");
    if (life.current) life.current.textContent = hours === null ? NO_VALUE : fmt(hours, 1);
    if (lifeNote.current) {
      lifeNote.current.textContent =
        hours === null
          ? diagnosed
            ? "no decline fitted"
            : ""
          : `${subsystem} · p10 ${fmt(rul?.p10 ?? Number.NaN, 2)} h`;
    }

    // No task when nothing fits. The advisory then falls to `degradedNote`, which
    // names the subsystem the rail is already showing rather than a repair on a
    // cylinder the catalogue guessed: commanded on cylinder 2, this panel told a
    // maintainer to replace injector 3.
    const task = diagnosed && !twin.diagnosis.unexplained ? TASKS[twin.diagnosis.best] : null;
    if (action.current) {
      const degraded = diagnosed ? degradedNote(twin.health, twin.ever_locked) : null;
      action.current.textContent = task
        ? `${task.action} · ${task.duration}`
        : diagnosed
          ? (degraded ?? "No action, every parameter is at nominal")
          : WAITING;
    }

    const detection = twin?.detection;
    const alarmed = detection?.drift_since ?? null;
    if (detected.current) {
      detected.current.textContent = alarmed === null ? "no alarm" : missionClock(alarmed);
    }
    if (redline.current) {
      const tripped = detection?.redline_since ?? null;
      redline.current.textContent = tripped === null ? "no trip" : missionClock(tripped);
      redline.current.toggleAttribute("data-alarm", tripped !== null);
    }
  });

  // `shrink-0` and no grow, never `flex-1`: a zero basis receives only positive
  // slack, so an overflowing alert stack left this panel at zero height. Constant
  // height whatever the log does; the log takes the slack and scrolls.
  return (
    <section className="marks relative flex shrink-0 flex-col overflow-hidden">
      <div className="border-border flex h-9 shrink-0 items-center justify-between border-b px-4">
        <span className="t-section">AI ADVISORY</span>
        <span className="border-structure-hi text-muted-foreground border border-dashed px-[7px] py-[3px] text-[10px] tracking-[0.08em]">
          ◊ INFERRED
        </span>
      </div>

      <div className="flex min-h-0 flex-1 p-3">
        <div className="border-structure flex min-h-0 flex-1 flex-col justify-between gap-[10px] border border-dashed px-[14px] py-3">
          <Field label="Diagnosis">
            <span className="text-[13px] leading-[1.45] text-pretty">
              <span ref={diagnosis}>{WAITING}</span>
              <span ref={confidence} className="text-foreground-dim ml-2" />
            </span>
          </Field>

          <Divider />

          <Field label="Remaining useful life">
            <div className="flex items-baseline gap-3">
              <span className="num t-value">
                <span ref={life}>{NO_VALUE}</span>
                <span className="text-muted-foreground ml-[5px] text-[11px]">h</span>
              </span>
              <span
                ref={lifeNote}
                className="num text-muted-foreground text-[11px] leading-[1.4]"
              />
            </div>
          </Field>

          <Divider />

          <Field label="Recommendation">
            <span ref={action} className="text-[13px] leading-[1.45] text-pretty">
              {WAITING}
            </span>
          </Field>

          {/* The measured comparison, in place of a risk percentage nothing
              computes. On the demonstration fault the right-hand cell reads
              `no trip`, and that is the entire argument for the product. */}
          <div className="border-border flex items-baseline justify-between gap-2 border-t pt-[10px]">
            <span className="label-micro">
              Twin alarm
              <span ref={detected} className="num text-foreground ml-[6px] text-[12px] normal-case">
                {NO_VALUE}
              </span>
            </span>
            <span className="label-micro">
              Certified redline
              <span ref={redline} className="num text-foreground ml-[6px] text-[12px] normal-case">
                {NO_VALUE}
              </span>
            </span>
          </div>
        </div>
      </div>
    </section>
  );
}
