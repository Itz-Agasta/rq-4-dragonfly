/**
 * Which channels carry the disagreement, ranked.
 *
 * Attribution is on the absolute residual in standard deviations, which is the
 * only comparison that survives channels being in different units: a kelvin and a
 * kilogram per hour cannot be ranked against each other, and their departures
 * from what a healthy engine would read can.
 *
 * Shares are whole percentages summing to exactly 100 by largest remainder. A
 * panel of percentages that sums to 99 is the first thing a sceptical reader
 * checks.
 *
 * **Gated on detection, for the reason isolation is.** Attribution asks which
 * channels carry the disagreement and never whether there is one, so on a healthy
 * engine it ranks noise: every channel sits under a sigma, the order changes
 * several times a second, and five bars reorder themselves while someone is
 * reading them. A magnitude threshold was tried first and is the wrong test at
 * twenty-two channels, because a standing model bias of 0.69 sigma on torque
 * alone puts the sum of the absolute residuals near eight on an engine with
 * nothing wrong with it.
 *
 * A counterfactual strip sat beneath this and was removed: it named the
 * second-shortest remaining life, so that `RULED OUT` could not be read as "those
 * subsystems are healthy". Nothing replaces it, so **a second, milder fault is
 * invisible here until the loud one is repaired.** The rail's per-subsystem
 * remaining life is where a reader has to go for it.
 */

import { useRef } from "react";

import { fmt } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import { isFresh } from "@/lib/telemetry";

/** Rows shown. Beyond five the shares are rounding noise. */
const ROWS = 5;

/** Header text while nothing is ranked. Names the column instead of scoring it. */
const SCORE_LABEL = "share of total |residual| in sigmas";

export function Attribution({ channels }: { channels: string[] }) {
  const rows = useRef<(HTMLDivElement | null)[]>([]);
  const note = useRef<HTMLSpanElement>(null);
  const score = useRef<HTMLSpanElement>(null);
  const empty = useRef<HTMLDivElement>(null);

  useLiveSink((frame) => {
    const twin = frame.twin;
    const fresh = isFresh(frame.ages.engine_ms);
    const d = twin?.detection;
    const fired = d !== undefined && !d.calibrating && (d.anomaly || d.drift);

    if (!twin || !fresh || !fired) {
      rows.current.forEach((row) => row && (row.style.display = "none"));
      if (empty.current) empty.current.style.display = "flex";
      if (note.current) {
        note.current.textContent =
          !twin || !fresh
            ? "waiting for the twin"
            : d?.calibrating
              ? "detector is baselining · nothing to attribute yet"
              : "no detector has fired · the residual is inside its band";
      }
      // Back to naming the column. The score belongs to the ranked rows, and
      // keeping the last alarm's total beside a panel showing none of them
      // attributes a number to channels that are no longer on screen.
      if (score.current) score.current.textContent = SCORE_LABEL;
      return;
    }
    const magnitudes = twin.normalised.map(Math.abs);
    if (empty.current) empty.current.style.display = "none";

    const ranked = magnitudes
      .map((v, i) => ({ v, i }))
      .toSorted((a, b) => b.v - a.v)
      .slice(0, ROWS);
    const shown = ranked.reduce((a, b) => a + b.v, 0);

    // Largest remainder over the rows actually displayed, so the column sums to
    // 100 rather than to whatever the top five happen to be worth.
    const exact = ranked.map((r) => (r.v / shown) * 100);
    const shares = exact.map(Math.floor);
    let left = 100 - shares.reduce((a, b) => a + b, 0);
    exact
      .map((v, i) => ({ frac: v - Math.floor(v), i }))
      .toSorted((a, b) => b.frac - a.frac)
      .forEach(({ i }) => {
        if (left > 0) {
          shares[i] += 1;
          left -= 1;
        }
      });

    rows.current.forEach((row, k) => {
      if (!row) return;
      const entry = ranked[k];
      if (!entry) {
        row.style.display = "none";
        return;
      }
      row.style.display = "";
      const name = row.querySelector<HTMLElement>("[data-name]");
      const share = row.querySelector<HTMLElement>("[data-share]");
      const sigma = row.querySelector<HTMLElement>("[data-sigma]");
      const bar = row.querySelector<HTMLElement>("[data-bar]");
      if (name) name.textContent = channels[entry.i] ?? `ch ${entry.i}`;
      if (share) share.textContent = `${shares[k]}%`;
      if (sigma) {
        const signed = twin.normalised[entry.i];
        sigma.textContent = `${signed > 0 ? "+" : ""}${fmt(signed, 2)} σ`;
      }
      if (bar) {
        bar.style.width = `${shares[k]}%`;
        // Only the leading channel is accented. Colour is alarm, and five
        // accented bars would say five things are wrong.
        bar.style.background = k === 0 ? "var(--primary)" : "var(--structure-hi)";
      }
    });

    if (score.current) {
      const total = magnitudes.reduce((a, b) => a + b, 0);
      score.current.textContent = `share of score ${fmt(total, 2)}`;
    }
  });

  return (
    <section className="cell cell--flush flex h-full min-h-0 min-w-0 flex-col">
      <header className="border-border flex shrink-0 items-baseline justify-between border-b px-4 py-3">
        <h2 className="t-section">Attribution · anomaly score</h2>
        <span ref={score} className="label-micro">
          {SCORE_LABEL}
        </span>
      </header>

      <div className="flex min-h-0 flex-1 flex-col px-4 py-2">
        {Array.from({ length: ROWS }, (_, i) => (
          <div
            key={i}
            ref={(el) => {
              rows.current[i] = el;
            }}
            className="border-border flex min-h-0 flex-1 items-center gap-3 border-b"
            style={{ display: "none" }}
          >
            <span data-name className="t-body w-[86px] shrink-0 truncate" />
            <span data-sigma className="num w-[78px] shrink-0 text-right text-[14px]" />
            <div className="bg-muted h-3 min-w-0 flex-1">
              <div data-bar className="tween h-full" style={{ width: "0%" }} />
            </div>
            <span
              data-share
              className="num text-muted-foreground w-[42px] shrink-0 text-right text-[11px]"
            />
          </div>
        ))}
        {/* The panel keeps its height whether or not a detector has fired, so
            with no bars there is a 300px void. The placeholder takes the whole of
            it rather than leaving one grey line adrift in the middle, which reads
            as a panel that failed to load. Dashed, the same border this screen
            uses everywhere a box is holding something it does not have yet. */}
        <div
          ref={empty}
          className="border-structure flex min-h-0 flex-1 flex-col items-center justify-center gap-2 border border-dashed px-[14px] py-[10px]"
        >
          <span ref={note} className="t-small text-muted-foreground" />
          <span className="label-micro">channel ranking appears when a detector fires</span>
        </div>
      </div>
    </section>
  );
}
