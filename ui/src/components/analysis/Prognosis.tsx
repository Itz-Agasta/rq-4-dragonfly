/**
 * How long the engine has, and what to do about it.
 *
 * The hero says what the projection rests on as well as what it is. On the
 * demonstration fault it reads a fraction of an hour, because `--fault-ramp 120`
 * cokes an injector in two minutes and that rate extrapolates to nine: printing
 * `0.15 h` alone invites a reader to conclude that injectors coke in nine minutes.
 * The driver, the fitted rate and the span the fit covers sit beside it so the
 * figure explains itself. A realistic rate needs a recorded mission replayed
 * through the twin.
 *
 * A mission-remaining and shortfall line is deliberately absent. Nothing on this
 * bus carries a planned mission length, so it could only be authored, and a
 * shortfall is the one number in this column an operator would act on directly.
 *
 * Rows are in `SUBSYSTEMS` order and do not move, for the reason the matrix rows
 * do not: this updates live and a table that sorts itself is unreadable exactly
 * while it is being read.
 */

import { useRef } from "react";

import { AdvisoryPanel } from "@/components/analysis/AdvisoryPanel";
import { AXIS_MAX_H, AXIS_TICKS, logX } from "@/components/analysis/prognosis";
import { Trajectory } from "@/components/analysis/Trajectory";
import { fmt, lifeRange, NO_VALUE, remainingLife } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import type { Parameter } from "@/lib/signatures";
import { isFresh, SUBSYSTEMS } from "@/lib/telemetry";

/** Fill of the limiting row's interval band. The only accent spent in this block. */
const BAND_LEAD = "rgb(255 107 53 / 0.32)";

export function Prognosis({ parameters }: { parameters: Parameter[] }) {
  const scope = useRef<HTMLSpanElement>(null);
  const hero = useRef<HTMLSpanElement>(null);
  const heroUnit = useRef<HTMLSpanElement>(null);
  const interval = useRef<HTMLSpanElement>(null);
  const basis = useRef<HTMLSpanElement>(null);
  const rows = useRef<(HTMLDivElement | null)[]>([]);

  useLiveSink((frame) => {
    const p = frame.prognosis;
    const fresh = isFresh(frame.ages.engine_ms);
    const limiting = fresh ? (p?.limiting ?? null) : null;
    const worst = limiting === null ? null : p?.subsystem[limiting];
    const life = worst?.hours ?? null;

    if (scope.current) {
      scope.current.textContent =
        limiting === null ? "· nothing is declining" : `· ${SUBSYSTEMS[limiting]!}`;
    }
    // Never a zero. `hours: null` means nothing is degrading, and a hero reading
    // 0.00 h grounds a serviceable aircraft.
    const shown = life === null ? { value: NO_VALUE, unit: "" } : remainingLife(life);
    if (hero.current) hero.current.textContent = shown.value;
    if (heroUnit.current) heroUnit.current.textContent = shown.unit;
    if (interval.current) {
      interval.current.textContent =
        worst && worst.p10 !== null ? `p10–p90  ${lifeRange(worst.p10, worst.p90)}` : "";
    }
    // Two ways to have no life to report and they are not the same claim: the
    // window is still filling, or it is full and nothing in it is declining. Any
    // parameter with a fit span answers which, since they share one window.
    if (basis.current) {
      const span = fresh ? (p?.parameter.find((r) => r.fit_span_s > 0)?.fit_span_s ?? 0) : 0;
      basis.current.textContent = worst
        ? `${worst.driver} falling ${fmt(worst.rate_per_hour, 4)}/h · fitted over ${fmt(worst.fit_span_s / 60, 1)} min`
        : span > 0
          ? `no parameter is declining · fitted over ${fmt(span / 60, 1)} min`
          : "building the trend window";
    }

    rows.current.forEach((row, s) => {
      if (!row) return;
      const rul = fresh ? p?.subsystem[s] : undefined;
      const hours = rul?.hours ?? null;
      const lead = s === limiting;

      const name = row.querySelector<HTMLElement>("[data-name]");
      const value = row.querySelector<HTMLElement>("[data-value]");
      const unit = row.querySelector<HTMLElement>("[data-unit]");
      const range = row.querySelector<HTMLElement>("[data-range]");
      const band = row.querySelector<HTMLElement>("[data-band]");
      const tick = row.querySelector<HTMLElement>("[data-tick]");

      if (name) name.style.color = lead ? "var(--primary)" : "var(--foreground)";

      if (rul === undefined || hours === null) {
        // A subsystem with no health parameter behind it cannot have a remaining
        // life, and calling that "no decline" would claim a measurement nobody
        // took. The two cases read differently on purpose.
        //
        // Wear stands without a projection: a stopped fault is genuinely not
        // declining and genuinely three quarters used, and a row printing only
        // the second half of that reads as a healthy subsystem.
        const spent = rul && rul.consumed >= 0.005;
        if (value) {
          value.textContent = spent
            ? `${fmt(rul.consumed * 100, 0)}% spent`
            : rul?.driver
              ? "no decline"
              : "no parameter";
          value.style.color = "var(--foreground-dim)";
          value.style.fontSize = "10px";
        }
        if (unit) unit.style.display = "none";
        if (range) range.textContent = spent ? "no decline" : "";
        if (band) band.style.display = "none";
        if (tick) tick.style.display = "none";
        return;
      }

      if (value) {
        const cell = remainingLife(Math.min(hours, AXIS_MAX_H));
        value.textContent = cell.value;
        value.style.color = lead ? "var(--primary)" : "var(--foreground)";
        value.style.fontSize = "14px";
        if (unit) {
          unit.textContent = cell.unit;
          unit.style.display = "";
        }
      }
      if (range) {
        range.textContent = rul.p10 === null ? "" : `[${lifeRange(rul.p10, rul.p90)}]`;
      }
      if (band && tick) {
        band.style.display = "";
        tick.style.display = "";
        const lo = logX(rul.p10 ?? hours);
        const hi = logX(rul.p90 ?? AXIS_MAX_H);
        band.style.left = `${lo * 100}%`;
        // A floor on the width so a tightly bounded interval is still a band and
        // not an invisible sliver behind its own p50 tick.
        band.style.width = `${Math.max(hi - lo, 0.006) * 100}%`;
        band.style.background = lead ? BAND_LEAD : "var(--border)";
        tick.style.left = `${logX(hours) * 100}%`;
        tick.style.background = lead ? "var(--primary)" : "var(--foreground)";
      }
    });
  });

  return (
    <section className="cell cell--flush flex h-full min-h-0 min-w-0 flex-col">
      <header className="border-border flex h-9 shrink-0 items-center justify-between border-b px-4">
        <h2 className="t-section">Prognosis</h2>
        <span className="label-micro">RUL · p50</span>
      </header>

      <div className="border-border shrink-0 border-b px-4 py-3">
        <div className="label-micro flex gap-1">
          <span className="shrink-0">remaining useful life</span>
          <span ref={scope} className="text-foreground-dim truncate" />
        </div>
        <div className="mt-1.5 flex items-baseline gap-2">
          <span ref={hero} className="num t-hero text-primary">
            {NO_VALUE}
          </span>
          <span ref={heroUnit} className="text-muted-foreground text-[11px]" />
          <span ref={interval} className="num text-foreground-dim ml-auto text-[11px]" />
        </div>
        <span ref={basis} className="t-small text-muted-foreground mt-2 block leading-relaxed" />
      </div>

      <div className="border-border shrink-0 border-b">
        <div className="px-4 pt-2 pb-1">
          <span className="label-micro">subsystem · remaining life</span>
          {/* Ticks on their own row rather than beside the caption: the floor
              tick's label sits at the very left of the track and would land
              underneath any caption sharing the line. The floor itself is
              `AXIS_MIN_H`, which carries why it sits where it does. */}
          <div className="relative mt-1 h-[13px]">
            {AXIS_TICKS.map((h, i) => (
              <span
                key={h}
                className="num text-foreground-dim absolute top-0 text-[10px] whitespace-nowrap"
                style={{
                  left: `${logX(h) * 100}%`,
                  transform: `translateX(${i === 0 ? "0" : i === AXIS_TICKS.length - 1 ? "-100%" : "-50%"})`,
                }}
              >
                {h >= 1000 ? "1k" : `${h} h`}
              </span>
            ))}
          </div>
        </div>

        {SUBSYSTEMS.map((label, s) => (
          <div
            key={label}
            ref={(el) => {
              rows.current[s] = el;
            }}
            className="border-border border-t px-4 py-[5px]"
          >
            <div className="flex items-baseline justify-between gap-2">
              <span data-name className="min-w-0 flex-1 truncate text-[12px]">
                {label}
              </span>
              <span className="flex shrink-0 items-baseline gap-1.5">
                <span data-value className="num text-[14px]">
                  {NO_VALUE}
                </span>
                <span data-unit className="text-muted-foreground text-[10px]" />
                <span
                  data-range
                  className="num text-foreground-dim text-right text-[10px] whitespace-nowrap"
                />
              </span>
            </div>
            {/* p10 to p90 as a filled band with p50 as a tick through it. The band
                is where the answer is not known; a bare marker would put a coin
                flip where a dispatch decision goes.

                Divs rather than an SVG line: `x1`/`x2` on a `<line>` are not CSS
                transitionable in Chrome, and these move once per readout, so
                without `.tween` they step five times a second. */}
            <div className="bg-muted relative mt-[3px] h-[6px] w-full">
              <div data-band className="tween absolute inset-y-0" style={{ display: "none" }} />
              <div
                data-tick
                className="tween absolute inset-y-[-1px] -ml-px w-[2px]"
                style={{ display: "none" }}
              />
            </div>
          </div>
        ))}
      </div>

      <Trajectory parameters={parameters} />
      {/* At 1440 the column has slack. It goes here rather than into the chart:
          a trajectory stretched to five hundred pixels shows a slowly declining
          parameter as a flat line with an enormous gap beneath it. */}
      <div className="min-h-0 flex-1" />
      <AdvisoryPanel />
    </section>
  );
}
