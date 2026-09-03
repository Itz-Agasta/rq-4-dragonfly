/**
 * What the twin thinks is wrong, and why it thinks the alternatives are not.
 *
 * The winner is expanded with its posterior and the evidence behind it; the
 * runners-up are dim, with the channel that rejected each one. The rejection is
 * computed from the observation rather than templated: it names the channel where
 * that hypothesis's own best fit disagrees most with what the engine is doing.
 *
 * Everything the library carries and did not choose is listed below the
 * runners-up. That block exists because the column has a void under it when only
 * one alternative has weight, and because four rows of a nine-row library read as
 * a shortlist someone drew up rather than as the whole library being scored.
 */

import { useRef } from "react";

import { fmt, NO_VALUE } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import type { Matrix } from "@/lib/signatures";
import { isFresh, SUBSYSTEMS } from "@/lib/telemetry";

/** Runners-up shown beneath the winner. Four rows fill the column at 900px. */
const ALTERNATES = 3;

/** Posterior below which a hypothesis is not worth a row of its own. */
const WORTH_SHOWING = 0.005;

export function Hypotheses({ data }: { data: Matrix }) {
  const headline = useRef<HTMLSpanElement>(null);
  const percent = useRef<HTMLSpanElement>(null);
  const bar = useRef<HTMLDivElement>(null);
  const kind = useRef<HTMLSpanElement>(null);
  const evidence = useRef<HTMLSpanElement>(null);
  const rows = useRef<(HTMLDivElement | null)[]>([]);
  const rejected = useRef<(HTMLDivElement | null)[]>([]);
  const spentName = useRef<HTMLSpanElement>(null);
  const spentValue = useRef<HTMLSpanElement>(null);
  const spentNote = useRef<HTMLSpanElement>(null);

  useLiveSink((frame) => {
    const twin = frame.twin;
    const fresh = isFresh(frame.ages.engine_ms);
    const diagnosis = twin?.diagnosis;

    if (!diagnosis || !fresh) {
      if (headline.current) headline.current.textContent = NO_VALUE;
      if (percent.current) percent.current.textContent = NO_VALUE;
      if (bar.current) bar.current.style.width = "0%";
      if (kind.current) kind.current.textContent = "";
      if (evidence.current) evidence.current.textContent = "waiting for the twin";
      rows.current.forEach((row) => row && (row.style.display = "none"));
      rejected.current.forEach((row) => row && (row.style.display = "none"));
      // Every sink this component owns is cleared here, the spent panel
      // included. Left alone it holds its last index beside three panels saying
      // the twin is gone, which reads as the one live number on the screen.
      if (spentName.current) spentName.current.textContent = NO_VALUE;
      if (spentValue.current) spentValue.current.textContent = NO_VALUE;
      if (spentNote.current) spentNote.current.textContent = "waiting for the twin";
      return;
    }

    const best = diagnosis.best;
    const posterior = diagnosis.posterior[best] ?? 0;
    if (headline.current) headline.current.textContent = data.hypotheses[best];
    if (percent.current) percent.current.textContent = `${fmt(posterior * 100, 1)}%`;
    if (bar.current) bar.current.style.width = `${Math.round(posterior * 100)}%`;
    if (kind.current) {
      kind.current.textContent = data.instrument[best]
        ? "instrument · distrust the reading, not the engine"
        : best === 0
          ? "no fault found"
          : "engine";
    }
    if (evidence.current) {
      const score = diagnosis.match_score[best];
      evidence.current.textContent =
        best === 0
          ? "residual is inside the band on every channel"
          : `residual pattern matches this signature at ${fmt(score, 3)}`;
    }

    // Alternates in descending posterior, skipping the winner. Recomputed per
    // frame but only ever written to four rows, so the cost is a sort of nine.
    const ranked = diagnosis.posterior
      .map((p, h) => ({ p, h }))
      .filter((x) => x.h !== best && x.p > WORTH_SHOWING)
      .toSorted((a, b) => b.p - a.p)
      .slice(0, ALTERNATES);

    rows.current.forEach((row, i) => {
      if (!row) return;
      const entry = ranked[i];
      if (!entry) {
        row.style.display = "none";
        return;
      }
      row.style.display = "";
      const name = row.querySelector<HTMLElement>("[data-name]");
      const value = row.querySelector<HTMLElement>("[data-percent]");
      const fill = row.querySelector<HTMLElement>("[data-bar]");
      const why = row.querySelector<HTMLElement>("[data-why]");
      if (name) name.textContent = data.hypotheses[entry.h];
      if (value) value.textContent = `${fmt(entry.p * 100, 1)}%`;
      if (fill) fill.style.width = `${Math.max(Math.round(entry.p * 100), 1)}%`;
      if (why) {
        const channel = diagnosis.rejection[entry.h];
        why.textContent = channel ? `rejected on ${channel}` : "";
      }
    });

    // Everything else the library carries, in the same descending order, so a
    // reader sees all nine being scored rather than a shortlist.
    const dismissed = diagnosis.posterior
      .map((p, h) => ({ p, h }))
      .filter((x) => x.h !== best && !ranked.some((r) => r.h === x.h))
      .toSorted((a, b) => b.p - a.p);

    rejected.current.forEach((row, i) => {
      if (!row) return;
      const entry = dismissed[i];
      if (!entry) {
        row.style.display = "none";
        return;
      }
      row.style.display = "";
      const name = row.querySelector<HTMLElement>("[data-name]");
      const why = row.querySelector<HTMLElement>("[data-why]");
      if (name) name.textContent = data.hypotheses[entry.h];
      if (why) why.textContent = diagnosis.rejection[entry.h] || "";
    });

    // What the estimator spent, which is a different question from what is
    // wrong. An index falls because the filter moved a parameter to explain the
    // residual, and with no parameter for misfire or for a lying probe it
    // reaches for the nearest one it has. The two panels then disagree, and the
    // diagnosis is the one to believe.
    let worst = -1;
    let lowest = Number.POSITIVE_INFINITY;
    twin.health.forEach((v, i) => {
      if (Number.isFinite(v) && v < lowest) {
        lowest = v;
        worst = i;
      }
    });
    if (spentName.current) {
      spentName.current.textContent = worst < 0 ? NO_VALUE : SUBSYSTEMS[worst];
    }
    if (spentValue.current) {
      spentValue.current.textContent = worst < 0 ? NO_VALUE : fmt(lowest, 0);
    }
    if (spentNote.current) {
      const agrees = worst >= 0 && data.subsystem[best] === worst;
      spentNote.current.textContent =
        best === 0
          ? "nothing spent; every parameter is at nominal"
          : // An instrument fault can never legitimately agree, whatever the
            // subsystem column says. The engine is serviceable and the reading
            // is not, so an index that has fallen is the filter bending a real
            // engine parameter to fit a lying probe. Reporting that as agreement
            // sends someone to open a healthy engine, which is the one outcome
            // `Hypothesis::instrument` exists to prevent.
            data.instrument[best]
            ? "spent on the engine for a fault that is not in the engine: the filter bent this parameter to fit a lying probe. Replace the instrument, not the part"
            : agrees
              ? "agrees with the diagnosis"
              : "disagrees with the diagnosis, which is expected: this fault has no parameter of its own, so the filter spends the nearest one it has";
    }
  });

  return (
    <section className="cell cell--flush flex h-full min-h-0 min-w-0 flex-col">
      <header className="border-border shrink-0 border-b px-4 py-3">
        <h2 className="t-section">Diagnosis</h2>
      </header>

      {/* 2px accent edge marks the selected hypothesis. Cheapest way to say
          which row the rest of the column is about without spending the hue on
          text. */}
      <div className="border-border relative shrink-0 border-b px-4 py-4">
        <span className="bg-primary absolute inset-y-0 left-0 w-[2px]" aria-hidden="true" />
        <span ref={kind} className="label-micro block" />
        <span ref={headline} className="t-section text-primary mt-1 block">
          {NO_VALUE}
        </span>
        <div className="mt-3 flex items-baseline justify-between">
          <span className="label-micro">posterior</span>
          <span ref={percent} className="num text-foreground text-[14px]">
            {NO_VALUE}
          </span>
        </div>
        <div className="bg-muted mt-2 h-[4px] w-full">
          <div ref={bar} className="tween bg-primary h-full" style={{ width: "0%" }} />
        </div>
        <span ref={evidence} className="t-small text-muted-foreground mt-3 block" />
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        {Array.from({ length: ALTERNATES }, (_, i) => (
          <div
            key={i}
            ref={(el) => {
              rows.current[i] = el;
            }}
            className="border-border border-b px-4 py-3"
            style={{ display: "none" }}
          >
            <div className="flex items-baseline justify-between gap-2">
              <span data-name className="t-small text-muted-foreground truncate" />
              <span data-percent className="num t-small text-foreground-dim shrink-0" />
            </div>
            <div className="bg-muted mt-2 h-[3px] w-full">
              <div data-bar className="tween bg-structure-hi h-full" style={{ width: "0%" }} />
            </div>
            <span data-why className="t-small text-foreground-dim mt-2 block italic" />
          </div>
        ))}

        <div className="px-4 pt-3 pb-2">
          <span className="label-micro block">
            ruled out · all {data.hypotheses.length} library hypotheses scored
          </span>
          {Array.from({ length: data.hypotheses.length - 1 }, (_, i) => (
            <div
              key={i}
              ref={(el) => {
                rejected.current[i] = el;
              }}
              className="mt-[6px] flex items-baseline justify-between gap-2"
              style={{ display: "none" }}
            >
              <span data-name className="label-micro text-foreground-dim truncate" />
              <span data-why className="label-micro shrink-0 truncate" />
            </div>
          ))}
        </div>
      </div>

      <div className="border-border shrink-0 border-t px-4 py-3">
        <span className="label-micro block">what the estimator spent</span>
        <div className="mt-2 flex items-baseline justify-between gap-2">
          <span ref={spentName} className="t-small text-muted-foreground truncate">
            {NO_VALUE}
          </span>
          <span ref={spentValue} className="num t-small text-foreground shrink-0">
            {NO_VALUE}
          </span>
        </div>
        <span ref={spentNote} className="t-small text-foreground-dim mt-2 block leading-relaxed" />
      </div>

      <footer className="border-border label-micro text-foreground-dim shrink-0 border-t px-4 py-2 leading-relaxed">
        posterior from a likelihood ratio over the signature matrix, with severity fitted per
        hypothesis. isolation runs only once detection has fired.
      </footer>
    </section>
  );
}
