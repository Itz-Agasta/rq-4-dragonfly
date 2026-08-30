/**
 * Subsystem health, seven rows.
 *
 * MOCK: values come from `./data`. See that file for what replaces them.
 *
 * Healthy rows are quiet: near-white numerals on the panel, no colour, no bars,
 * no sparklines. All seven indices sit between 72 and 99, so a bar would be seven
 * nearly-identical near-full rectangles carrying no information. The one row that
 * matters gets the accent and a 2px edge, and that contrast is the whole point.
 */

import { SUBSYSTEMS } from "@/components/ops/data";

export function HealthRail() {
  return (
    <section className="border-border flex w-[220px] shrink-0 flex-col border-r">
      <div className="border-border flex h-7 shrink-0 items-center border-b px-[14px]">
        <span className="label-micro">Subsystem health</span>
      </div>

      {SUBSYSTEMS.map((s) => (
        <div
          key={s.name}
          className="border-border relative flex h-[52px] shrink-0 items-center justify-between gap-2 border-b px-[14px]"
        >
          {s.degrading ? (
            <span
              className="bg-primary absolute top-0 bottom-0 left-0 w-[2px]"
              aria-hidden="true"
            />
          ) : null}
          <span className="text-[12px] tracking-[0.02em] whitespace-nowrap">{s.name}</span>
          <span
            className={`num text-[22px] leading-[1.1] ${
              s.degrading ? "text-primary" : "text-foreground"
            }`}
          >
            {s.value}
          </span>
        </div>
      ))}

      <div className="min-h-0 flex-1" />
    </section>
  );
}
