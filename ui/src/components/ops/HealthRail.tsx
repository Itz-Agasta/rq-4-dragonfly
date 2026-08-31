/**
 * Subsystem health, seven rows, live from the twin.
 *
 * Healthy rows are quiet: near-white numerals on the panel, no colour, no bars,
 * no sparklines. Seven indices that all sit in the nineties would make a bar
 * chart of seven nearly-identical near-full rectangles carrying no information.
 * One row is accented and that contrast is the whole point.
 *
 * The accent is earned per frame from the live estimate rather than declared in
 * a constant: the lowest-scoring subsystem is marked, and only when it has
 * actually fallen. Run the simulator without a fault and nothing is accented,
 * which is the state a health display should be in almost all of the time.
 *
 * Each row also carries the quantity that produced its number. An index nobody
 * can interrogate is a score, and a score is what this system exists to replace.
 */

import { useRef } from "react";

import { fmt, NO_VALUE } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import { isFresh, SUBSYSTEM_INFERRED, SUBSYSTEMS } from "@/lib/telemetry";

/**
 * Index below which a subsystem is worth pointing at.
 *
 * A healthy engine settles with its worst subsystem in the mid nineties, because
 * the estimator's own noise moves the parameters by a fraction of a percent and
 * the indices are linear in that. It dips to the high eighties for a few seconds
 * after the twin re-seeds, which happens whenever the bus comes back, so the
 * threshold has to clear that transient as well as the settled value. Eighty does
 * both and is still far above the fifty-six a coked injector produces.
 */
const DEGRADED_BELOW = 80;

/** Row height, so the seven rows and the header fill the rail exactly. */
const ROW_H = 52;

export function HealthRail() {
  const rows = useRef<(HTMLDivElement | null)[]>([]);
  const values = useRef<(HTMLSpanElement | null)[]>([]);
  const drivers = useRef<(HTMLSpanElement | null)[]>([]);

  useLiveSink((frame) => {
    const twin = frame.twin;
    const fresh = isFresh(frame.ages.engine_ms);
    // The lowest index, and only if it has fallen far enough to mean something.
    // Marking whichever happens to be lowest regardless of value would put an
    // accent on a healthy engine every time, which teaches an operator to ignore
    // it.
    let worst = -1;
    if (twin) {
      let lowest = DEGRADED_BELOW;
      twin.health.forEach((v, i) => {
        if (Number.isFinite(v) && v < lowest) {
          lowest = v;
          worst = i;
        }
      });
    }

    SUBSYSTEMS.forEach((_, i) => {
      const value = twin?.health[i];
      const text = value === undefined || !Number.isFinite(value) ? NO_VALUE : fmt(value, 0);
      const el = values.current[i];
      if (el && el.textContent !== text) el.textContent = text;

      const row = rows.current[i];
      if (row) {
        const alarm = i === worst && fresh;
        if (alarm !== row.hasAttribute("data-alarm")) row.toggleAttribute("data-alarm", alarm);
        if (!fresh !== row.hasAttribute("data-stale")) row.toggleAttribute("data-stale", !fresh);
      }

      const driver = drivers.current[i];
      if (driver) {
        const name = twin?.health_driver[i];
        const at = twin?.health_driver_value[i];
        const limit = twin?.health_driver_limit[i];
        const next =
          !name || at === undefined || limit === undefined || !Number.isFinite(at)
            ? ""
            : `${name} ${fmt(at, at >= 100 ? 1 : 3)} / ${fmt(limit, limit >= 100 ? 1 : 3)}`;
        if (driver.textContent !== next) driver.textContent = next;
      }
    });
  });

  return (
    <section className="border-border flex w-[220px] shrink-0 flex-col border-r">
      <div className="border-border flex h-7 shrink-0 items-center border-b px-[14px]">
        <span className="label-micro">Subsystem health</span>
      </div>

      {SUBSYSTEMS.map((name, i) => (
        <div
          key={name}
          ref={(el) => {
            rows.current[i] = el;
          }}
          className="border-border group relative flex shrink-0 flex-col justify-center border-b px-[14px]"
          style={{ height: ROW_H }}
        >
          <span className="bg-primary absolute top-0 bottom-0 left-0 w-[2px] opacity-0 group-data-[alarm]:opacity-100" />
          <div className="flex items-center justify-between gap-2">
            <span className="text-[12px] tracking-[0.02em] whitespace-nowrap">
              {name}
              {/* Two of the seven are limit checks on channels the model does not
                  describe, so they carry no inference mark. */}
              {SUBSYSTEM_INFERRED[i] ? (
                <span className="text-structure ml-[5px] text-[9px]" title="inferred by the twin">
                  ◊
                </span>
              ) : null}
            </span>
            <span
              ref={(el) => {
                values.current[i] = el;
              }}
              className="num group-data-[alarm]:text-primary group-data-[stale]:text-foreground-dim text-[22px] leading-[1.1]"
            >
              {NO_VALUE}
            </span>
          </div>
          <span
            ref={(el) => {
              drivers.current[i] = el;
            }}
            className="label-micro text-structure mt-[3px] truncate"
          />
        </div>
      ))}

      <div className="min-h-0 flex-1" />
    </section>
  );
}
