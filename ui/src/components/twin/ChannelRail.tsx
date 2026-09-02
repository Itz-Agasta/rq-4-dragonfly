/**
 * Every compared channel, loudest disagreement first.
 *
 * The rail answers "where is the twin unhappy" before anyone has chosen what to
 * look at, and choosing a row is how the hero cell is chosen.
 *
 * # Rows are reordered without React
 *
 * Telemetry never becomes React state, so the twenty-two rows are rendered once
 * and the render loop writes each one's `order` on the flex container. The DOM
 * order is fixed and only the visual order moves, which also means a row keeps
 * its identity across a reorder: the click handler on a row is always the channel
 * whose name is printed in it.
 *
 * The number is a five second mean rather than this frame's value, and it is
 * also what the order is taken on; `store/twin.ts` carries why. The cells keep
 * the instantaneous residual, which is a different panel doing a different job.
 */

import { useRef } from "react";

import { NO_VALUE, signed } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import { isFresh } from "@/lib/telemetry";
import { COMPARED } from "@/store/compared";
import { telemetry } from "@/store/telemetry";
import { BAND_SIGMA } from "@/store/twin";

export interface ChannelRailProps {
  /** Index of the channel in the hero cell. */
  selected: number;
  /** Called when a row is chosen. */
  onSelect: (index: number) => void;
}

/** Scratch for the sort, allocated once rather than per readout. */
const ORDER = COMPARED.map((_, i) => i);

export function ChannelRail({ selected, onSelect }: ChannelRailProps) {
  const rows = useRef<(HTMLButtonElement | null)[]>([]);
  const values = useRef<(HTMLSpanElement | null)[]>([]);

  useLiveSink((frame) => {
    const history = telemetry.twin;
    const live = frame.twin !== null && isFresh(frame.ages.engine_ms);

    ORDER.sort((a, b) => Math.abs(history.smoothed(b)) - Math.abs(history.smoothed(a)));

    for (let position = 0; position < ORDER.length; position += 1) {
      const i = ORDER[position]!;
      const row = rows.current[i];
      const value = values.current[i];
      if (row) row.style.order = String(position);
      if (!value) continue;

      const sigma = live ? history.smoothed(i) : Number.NaN;
      const text = signed(sigma, 2);
      if (value.textContent !== text) value.textContent = text;
      const alarm = Number.isFinite(sigma) && Math.abs(sigma) > BAND_SIGMA;
      value.toggleAttribute("data-alarm", alarm);
      // A separate attribute from the readout's, because the global `data-alarm`
      // rule tints everything it contains and the channel name must stay white.
      row?.toggleAttribute("data-out", alarm);
    }
  });

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col overflow-hidden">
      <div className="border-border flex h-[28px] shrink-0 items-center justify-between border-b px-[14px]">
        <span className="label-micro">Channels</span>
        {/* Lower case sigma, so it is not uppercased into a summation sign by
            the label class, and the averaging window named rather than implied. */}
        <span className="label-micro normal-case">|σ| 5 s ↓</span>
      </div>
      <div className="flex min-h-0 flex-1 flex-col">
        {COMPARED.map((ch, i) => (
          <button
            key={ch.name}
            type="button"
            ref={(el) => {
              rows.current[i] = el;
            }}
            onClick={() => onSelect(i)}
            aria-pressed={i === selected}
            className={`group border-border relative flex min-h-[28px] flex-1 items-center justify-between gap-2 border-b px-[14px] text-left ${
              i === selected ? "bg-card" : "hover:bg-accent"
            }`}
          >
            {/* The one accent the rail is allowed, and it keeps the alarm
                meaning: a channel outside its own tolerance band. Selection is
                carried by the panel fill instead, so the two never compete. */}
            <span className="bg-primary absolute top-0 bottom-0 left-0 hidden w-[2px] group-data-[out]:block" />
            <span className="truncate text-[12px] tracking-[0.02em]">{ch.name}</span>
            <span className="num shrink-0 text-[14px]">
              <span
                ref={(el) => {
                  values.current[i] = el;
                }}
              >
                {NO_VALUE}
              </span>
              <span className="text-muted-foreground ml-[2px] text-[10px]">σ</span>
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}
