/**
 * The AI advisory panel: what the twin thinks is wrong, and what to do.
 *
 * Live, and reading the same diagnosis and prognosis ANALYSIS reads. Nothing here
 * is authored: two screens describing one engine have to agree, so every figure
 * comes off the wire and is formatted by the same function ANALYSIS calls.
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
 *
 * Two timestamps side by side are not an argument, so the block names each
 * method and spells its conclusion out on a line of its own. The verdict covers
 * the case where the redline wins, which on this fault cannot happen and on
 * another one can: a panel that can only report a win is an advert.
 */

import { useRef } from "react";

import { degradedNote, TASKS } from "@/lib/advisory";
import { lifeHours, missionClock, NO_VALUE, posteriorPct, remainingLife } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import { HYPOTHESES, isFresh, SUBSYSTEMS } from "@/lib/telemetry";
const WAITING = "waiting for the twin";

/**
 * Mirrored by hand from `twin_core::detect::Redline::tripped`: oil temperature,
 * both oil pressure bounds, coolant, and four cylinders each of EGT and CHT.
 * Stated so `no trip` reads as a monitor that ran, not one never wired up.
 */
const REDLINE_LIMITS = 12;

/** A span in `MM:SS`, or `H:MM:SS` past an hour. */
function span(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds));
  const h = Math.floor(whole / 3600);
  const m = Math.floor((whole % 3600) / 60);
  const s = whole % 60;
  const mm = h > 0 ? m.toString().padStart(2, "0") : m.toString();
  return `${h > 0 ? `${h}:` : ""}${mm}:${s.toString().padStart(2, "0")}`;
}

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
  const lifeUnit = useRef<HTMLSpanElement>(null);
  const lifeNote = useRef<HTMLSpanElement>(null);
  const action = useRef<HTMLSpanElement>(null);
  const detected = useRef<HTMLSpanElement>(null);
  const redline = useRef<HTMLSpanElement>(null);
  const verdict = useRef<HTMLDivElement>(null);

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
    const shown = hours === null ? { value: NO_VALUE, unit: "" } : remainingLife(hours);
    if (life.current) life.current.textContent = shown.value;
    if (lifeUnit.current) lifeUnit.current.textContent = shown.unit;
    if (lifeNote.current) {
      lifeNote.current.textContent =
        hours === null
          ? diagnosed
            ? "no decline fitted"
            : ""
          : `${subsystem} · p10 ${lifeHours(rul?.p10 ?? Number.NaN)}`;
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

    // Gated on `diagnosed` for the reason the diagnosis field is: before the twin
    // has looked, `no alarm` beside `no trip` is not two methods agreeing the
    // engine is well, it is two methods that have not run.
    const detection = diagnosed ? twin.detection : undefined;
    const alarmed = detection?.drift_since ?? null;
    const tripped = detection?.redline_since ?? null;
    if (detected.current) {
      detected.current.textContent = !detection
        ? NO_VALUE
        : alarmed === null
          ? "no alarm"
          : missionClock(alarmed);
      // Accented off the live `drift`, never off the stamp: the stamp is latched
      // for the lead time and would leave the panel alarming after a recovery.
      detected.current.toggleAttribute("data-alarm", detection?.drift ?? false);
    }
    if (redline.current) {
      redline.current.textContent = !detection
        ? NO_VALUE
        : tripped === null
          ? "no trip"
          : missionClock(tripped);
      redline.current.toggleAttribute("data-alarm", tripped !== null);
    }
    if (verdict.current) {
      // `lead_time_s` is the wire's own difference of the two stamps. Recomputing
      // it here would be a second answer to one question.
      const lead = detection?.lead_time_s ?? null;
      const text = !detection
        ? ""
        : detection.calibrating
          ? "detector still calibrating"
          : alarmed === null && tripped === null
            ? "neither method has alarmed"
            : alarmed !== null && tripped === null
              ? "only the twin fired"
              : alarmed === null
                ? "redline only, the twin did not fire"
                : lead !== null && lead > 0
                  ? `twin led the redline by ${span(lead)}`
                  : `redline first by ${span(-(lead ?? 0))}`;
      verdict.current.textContent = text;
      // Accent only where the twin is the one that found it. A redline win is
      // reported in the same words and left grey.
      verdict.current.toggleAttribute(
        "data-alarm",
        alarmed !== null && (tripped === null || (lead !== null && lead > 0)),
      );
    }
  });

  // `shrink-0` and no grow, never `flex-1`: a zero basis receives only positive
  // slack, so an overflowing alert stack left this panel at zero height. Constant
  // height whatever the log does; the log takes the slack and scrolls. The `lh`
  // minimums below reserve the second line the prose wraps to, so the card does
  // not jump up the rail the moment a diagnosis arrives.
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
            <span className="block min-h-[2lh] text-[13px] leading-[1.45] text-pretty">
              <span ref={diagnosis}>{WAITING}</span>
              <span ref={confidence} className="text-foreground-dim ml-2" />
            </span>
          </Field>

          <Divider />

          <Field label="Remaining useful life">
            <div className="flex items-baseline gap-3">
              <span className="num t-value">
                <span ref={life}>{NO_VALUE}</span>
                <span ref={lifeUnit} className="text-muted-foreground ml-[5px] text-[11px]" />
              </span>
              <span
                ref={lifeNote}
                className="num text-muted-foreground text-[11px] leading-[1.4]"
              />
            </div>
          </Field>

          <Divider />

          <Field label="Recommendation">
            <span ref={action} className="block min-h-[2lh] text-[13px] leading-[1.45] text-pretty">
              {WAITING}
            </span>
          </Field>

          {/* The measured comparison, in place of a risk percentage nothing
              computes. On the demonstration fault the redline row reads `no trip`
              for the whole flight, which is the entire argument for the product,
              so the verdict line states it rather than leaving a reader to
              notice an absence. */}
          <div className="border-border flex flex-col gap-[6px] border-t pt-[10px]">
            <div className="label-micro flex items-baseline justify-between gap-2">
              <span>
                Certified redline
                <span className="text-foreground-dim ml-[6px] normal-case">
                  {REDLINE_LIMITS} limits
                </span>
              </span>
              <span ref={redline} className="num text-foreground text-[12px] normal-case">
                {NO_VALUE}
              </span>
            </div>
            <div className="label-micro flex items-baseline justify-between gap-2">
              <span>
                Digital twin
                <span className="text-foreground-dim ml-[6px] normal-case">residual</span>
              </span>
              <span ref={detected} className="num text-foreground text-[12px] normal-case">
                {NO_VALUE}
              </span>
            </div>
            <div
              ref={verdict}
              className="text-muted-foreground min-h-[1lh] text-[11px] tracking-[0.08em] uppercase"
            />
          </div>
        </div>
      </div>
    </section>
  );
}
