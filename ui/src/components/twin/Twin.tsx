/**
 * TWIN: the measurement against the physics, channel by channel.
 *
 * Fixed 200px rail, fixed 300px right column, fluid centre. The same arrangement
 * as OPS and ANALYSIS, and for the same reason: the plots are the thing that has
 * to grow with the viewport, because a trace pair only argues anything if the two
 * lines have room to separate.
 *
 * # The hero cell follows the selection
 *
 * Screens are drill-downs rather than tabs. Arriving here from an OPS callout or
 * from the rail selects that channel and gives it the tall cell, so an operator
 * follows a fault instead of navigating to it. Everything else on the screen is
 * unchanged by the choice, which is what makes it safe to change: the rail, the
 * sync table and the health parameters describe the whole engine either way.
 */

import { ChannelRail } from "@/components/twin/ChannelRail";
import { HealthParams } from "@/components/twin/HealthParams";
import { PairedCell } from "@/components/twin/PairedCell";
import { SyncQuality } from "@/components/twin/SyncQuality";
import { useMatrix } from "@/lib/signatures";
import { useApp } from "@/store/app";
import { comparedIndex, COMPARED } from "@/store/compared";
import { HISTORY_SECONDS } from "@/store/telemetry";

/** Channel the screen opens on when nothing has been selected. */
const DEFAULT = "EGT 3";

/**
 * The context cells beside the hero, in preference order.
 *
 * Not arbitrary: exhaust temperature on a healthy cylinder is what makes a sick
 * one legible, fuel flow is the channel that separates a coked injector from a
 * misfire, and torque is where a loss of power shows regardless of cause. The
 * fourth is the spare, used when the hero is one of the first three; the hero can
 * displace at most one, so the list never needs a fifth.
 */
const CONTEXT = ["EGT 1", "FUEL", "TORQUE", "CHT 3"];

/** Rows the hero cell is worth against one context cell. */
const HERO_GROW = 2.2;

export function Twin() {
  const data = useMatrix();
  const channel = useApp((s) => s.selection.channel);
  const select = useApp((s) => s.select);

  const hero = comparedIndex(channel) ?? comparedIndex(DEFAULT) ?? 0;

  // Rebuilt on every render, which is every selection change and nothing else:
  // telemetry does not pass through React here.
  const cells = [hero];
  for (const name of CONTEXT) {
    if (cells.length === 4) break;
    const i = comparedIndex(name);
    if (i !== null && !cells.includes(i)) cells.push(i);
  }

  return (
    <div className="lattice min-h-0 min-w-0 flex-1 grid-cols-[200px_minmax(0,1fr)_300px]">
      <div className="cell cell--flush min-h-0">
        <ChannelRail
          selected={hero}
          onSelect={(i) => select({ channel: COMPARED[i]?.name ?? null })}
        />
      </div>

      <div className="cell cell--flush marks relative flex min-h-0 min-w-0 flex-col">
        <div className="border-border flex h-[40px] shrink-0 items-center justify-between gap-6 border-b px-[18px]">
          <span className="flex min-w-0 items-baseline gap-3">
            <span className="t-section whitespace-nowrap">MEASURED vs PHYSICS TWIN</span>
            {/* Stated once for the screen rather than once per cell: four cells
                on one time base with one cursor is one window, and repeating it
                four times says there might be four. */}
            <span className="label-micro whitespace-nowrap">{HISTORY_SECONDS} s window</span>
          </span>
          <div className="flex shrink-0 items-center gap-6">
            <span className="flex items-center gap-[9px] text-[11px] tracking-[0.06em] whitespace-nowrap">
              <svg width="42" height="8" viewBox="0 0 42 8" aria-hidden="true">
                <line x1="0" y1="4" x2="42" y2="4" stroke="var(--measured)" strokeWidth="2" />
              </svg>
              MEASURED
            </span>
            <span className="text-muted-foreground flex items-center gap-[9px] text-[11px] tracking-[0.06em] whitespace-nowrap">
              <svg width="42" height="8" viewBox="0 0 42 8" aria-hidden="true">
                <line
                  x1="0"
                  y1="4"
                  x2="42"
                  y2="4"
                  stroke="var(--predicted)"
                  strokeWidth="1.5"
                  strokeDasharray="6 4"
                />
              </svg>
              TWIN PREDICTED
            </span>
            <span className="text-muted-foreground flex items-center gap-[9px] text-[11px] tracking-[0.06em] whitespace-nowrap">
              <svg width="42" height="10" viewBox="0 0 42 10" aria-hidden="true">
                <rect x="0" y="2" width="42" height="6" fill="var(--muted)" />
                <line x1="0" y1="2" x2="42" y2="2" stroke="var(--border)" strokeWidth="1" />
                <line x1="0" y1="8" x2="42" y2="8" stroke="var(--border)" strokeWidth="1" />
              </svg>
              RESIDUAL ±3σ
            </span>
          </div>
        </div>

        {cells.map((index, position) => (
          <PairedCell
            key={COMPARED[index]?.name ?? index}
            index={index}
            grow={position === 0 ? HERO_GROW : 1}
            syncKey="twin"
          />
        ))}
      </div>

      <div className="cell cell--flush flex min-h-0 min-w-0 flex-col">
        <div className="border-border flex min-h-0 flex-[3] flex-col border-b">
          <SyncQuality />
        </div>
        <div className="flex min-h-0 flex-[2] flex-col">
          {data ? (
            <HealthParams parameters={data.parameters} />
          ) : (
            <div className="flex h-full items-center justify-center">
              <span className="label-micro">waiting for the parameter descriptors</span>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
