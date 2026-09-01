/**
 * The structured residual matrix.
 *
 * Nine hypotheses by twenty-two channels, each row the direction that fault
 * pushes the residual. Beneath them, outlined in the accent, the residual the
 * engine is producing right now. The screen's whole argument is that you can see
 * which row the bottom one matches, and see the one cell where it does not.
 *
 * The nine rows are constant and rendered once. Only the observed row, the match
 * scores and the winner's marker change per frame, and those are written to the
 * DOM inside the shared render loop rather than through React.
 */

import { useRef } from "react";

import type { Matrix as MatrixData } from "@/components/analysis/signatures";
import { ramp, sign } from "@/components/analysis/signatures";
import { fmt, NO_VALUE } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import { isFresh } from "@/lib/telemetry";

/** Width of the right-aligned hypothesis labels, per `design.md`. */
const LABEL_W = 178;
/** Width of the match-score column. */
const SCORE_W = 46;
/**
 * Row height, fixed, with the whole block centred in whatever height the panel
 * has. Letting the rows share the panel's height was tried and is worse: they
 * cap out, the leftover collects beneath them, and `OBSERVED NOW` ends up
 * stranded at the bottom of the panel. It has to sit directly under the
 * signatures, because comparing it against them is the only thing this is for.
 */
const ROW_H = 32;

/**
 * One grid template, shared by the labels, every hypothesis row and the observed
 * row, so the columns cannot drift apart.
 *
 * `minmax(0, 1fr)` on the channels rather than a percentage width with a minimum:
 * a minimum turns into a horizontal scrollbar the moment the viewport is narrow,
 * and a matrix you have to scroll sideways is a matrix you cannot compare rows
 * in, which is the only thing it is for.
 */
function template(channels: number): string {
  return `2px ${LABEL_W}px repeat(${channels}, minmax(0, 1fr)) ${SCORE_W}px`;
}

/**
 * Rows are in catalogue order and do not move.
 *
 * `docs/design/PUNCHLIST.md` asks for rows sorted by match score descending, and
 * that is right for the static mock it was written about. On a 20 Hz screen it is
 * not: match scores wander with noise, so a sorted table reorders itself several
 * times a second and becomes unreadable exactly when an operator is trying to
 * read it. A fixed order is also learnable, which a sorted one never is.
 *
 * The winner is marked instead, with an accent edge on its row, so the
 * comparison the sort existed to make is still one glance.
 */
export function Matrix({ data }: { data: MatrixData }) {
  const observed = useRef<(HTMLDivElement | null)[]>([]);
  const scores = useRef<(HTMLSpanElement | null)[]>([]);
  const marks = useRef<(HTMLDivElement | null)[]>([]);

  useLiveSink((frame) => {
    const twin = frame.twin;
    const fresh = isFresh(frame.ages.engine_ms);
    const diagnosis = twin?.diagnosis;

    // The observed row is normalised to unit length so it is on the same scale as
    // the signatures above it. Without that a settled fault at eight sigma paints
    // every cell white and matches nothing by eye.
    let unit: number[] | null = null;
    if (twin && fresh) {
      const norm = Math.hypot(...twin.normalised);
      if (norm > 0) unit = twin.normalised.map((v) => v / norm);
    }

    observed.current.forEach((cell, i) => {
      if (!cell) return;
      const v = unit?.[i];
      cell.style.background = v === undefined ? "var(--muted)" : ramp(v);
      const edge = v === undefined ? null : sign(v);
      cell.style.borderTopColor = edge === "top" ? "var(--structure-hi)" : "transparent";
      cell.style.borderBottomColor = edge === "bottom" ? "var(--structure-hi)" : "transparent";
    });

    scores.current.forEach((slot, h) => {
      if (!slot) return;
      const score = diagnosis?.match_score[h];
      slot.textContent = score === undefined || !fresh || h === 0 ? NO_VALUE : fmt(score, 2);
    });

    marks.current.forEach((mark, h) => {
      if (!mark) return;
      const won = fresh && diagnosis !== undefined && diagnosis.best === h;
      mark.style.background = won ? "var(--primary)" : "transparent";
    });
  });

  return (
    <section className="cell cell--flush marks relative flex h-full min-h-0 min-w-0 flex-col overflow-hidden">
      <header className="border-border flex shrink-0 items-center justify-between gap-5 border-b px-4 py-3">
        <h2 className="t-section">Structured residual matrix</h2>
        <div className="flex shrink-0 items-center gap-2.5">
          <span className="label-micro">expression</span>
          {/* The legend is the ramp itself. A written range makes a reader
              translate; the gradient lets them match a cell against it. */}
          <div
            className="h-2 w-24"
            style={{ background: "linear-gradient(90deg, #0a0a0b, #55555c, #f0f0f2)" }}
            aria-hidden="true"
          />
          <span className="num text-muted-foreground text-[10px] tracking-[0.06em]">
            0 → 1 σ-norm
          </span>
        </div>
      </header>

      <div className="flex min-h-0 min-w-0 flex-1 flex-col justify-center overflow-hidden px-4 py-3">
        <div className="flex min-w-0 flex-col">
          <ColumnLabels data={data} />
          <div className="flex flex-col">
            {data.hypotheses.map((name, h) => (
              <div
                key={name}
                className="grid items-stretch"
                style={{
                  height: ROW_H,
                  gridTemplateColumns: template(data.channels.length),
                }}
              >
                <div
                  aria-hidden
                  ref={(el) => {
                    marks.current[h] = el;
                  }}
                />
                <span className="t-small text-muted-foreground self-center truncate pr-3 text-right">
                  {name}
                </span>
                {data.rows[h].map((v, i) => (
                  <Cell key={data.channels[i]} value={v} />
                ))}
                <span
                  ref={(el) => {
                    scores.current[h] = el;
                  }}
                  className="num t-small text-foreground self-center pl-3 text-right"
                >
                  {NO_VALUE}
                </span>
              </div>
            ))}
          </div>

          <div
            className="mt-3 grid shrink-0 items-stretch outline outline-[var(--primary)]"
            style={{
              height: ROW_H,
              outlineOffset: 2,
              gridTemplateColumns: template(data.channels.length),
            }}
          >
            <div aria-hidden />
            <span className="t-small text-primary self-center pr-3 text-right">OBSERVED NOW</span>
            {data.channels.map((channel, i) => (
              <div
                key={channel}
                ref={(el) => {
                  observed.current[i] = el;
                }}
                className="tween min-w-0 border-y-2 border-transparent"
                style={{ background: "var(--muted)" }}
              />
            ))}
            <span />
          </div>
        </div>
      </div>

      <footer className="border-border text-foreground-dim label-micro shrink-0 border-t px-4 py-2">
        cell luminance is magnitude · cap on the upper edge is a rise, lower edge a fall
      </footer>
    </section>
  );
}

/** One matrix cell. Static: a signature does not change while the app runs. */
function Cell({ value }: { value: number }) {
  const edge = sign(value);
  return (
    <div
      className="min-w-0 border-y-2"
      style={{
        background: ramp(value),
        borderTopColor: edge === "top" ? "var(--structure-hi)" : "transparent",
        borderBottomColor: edge === "bottom" ? "var(--structure-hi)" : "transparent",
      }}
    />
  );
}

/** Rotated 10px channel names above the grid. */
function ColumnLabels({ data }: { data: MatrixData }) {
  return (
    <div
      className="grid items-end"
      style={{ height: 78, gridTemplateColumns: template(data.channels.length) }}
    >
      <div aria-hidden />
      <div />
      {data.channels.map((channel) => (
        <div key={channel} className="flex min-w-0 justify-center">
          <span
            className="label-micro whitespace-nowrap"
            style={{ writingMode: "vertical-rl", transform: "rotate(180deg)" }}
          >
            {channel}
          </span>
        </div>
      ))}
      <span className="label-micro self-end pl-3 text-right">match</span>
    </div>
  );
}
