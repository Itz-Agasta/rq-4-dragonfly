/**
 * The 40px mission bar, from the rail's right edge to the viewport edge.
 *
 * It carries no navigation. The rail owns that, which frees every pixel here for
 * state that is relevant on all six screens.
 *
 * Every value is a {@link Live} span written from the render loop, so the bar
 * updates at 20 Hz without React seeing a single re-render.
 */

import { fmt, missionClock, NO_VALUE } from "@/lib/fmt";
import { Live } from "@/lib/live";
import { isFresh } from "@/lib/telemetry";
import { linkIsUp, twinIsLocked, useApp } from "@/store/app";
import { channel } from "@/store/frame";

const AIRFRAME = "TAPAS-AF07";

function Rule() {
  return <span className="bg-border w-px self-stretch" aria-hidden="true" />;
}

function Readout({ id }: { id: string }) {
  const ch = channel(id);
  return (
    <div className="flex flex-col justify-center gap-[2px] px-[14px]">
      <span className="label-micro">{ch.label}</span>
      <span className="num text-[14px] leading-none whitespace-nowrap">
        <Live
          select={(f) => fmt(ch.get(f), ch.dp)}
          fresh={(f) => isFresh(f.ages[ch.source])}
          className="data-[stale]:text-foreground-dim"
        />
        <span className="text-muted-foreground ml-1 text-[10px]">{ch.unit}</span>
      </span>
    </div>
  );
}

export function TopBar({ screen }: { screen: string }) {
  const up = useApp(linkIsUp);
  const socket = useApp((s) => s.socket);
  const locked = useApp(twinIsLocked);

  return (
    <header className="border-border flex h-10 min-w-0 shrink-0 items-stretch border-b">
      <div className="flex shrink-0 items-center gap-[13px] pr-4 pl-6">
        <h1 className="text-[16px] leading-none font-medium tracking-[0.12em]">{screen}</h1>
        <Rule />
        <span className="text-muted-foreground text-[11px] tracking-[0.06em]">{AIRFRAME}</span>
      </div>

      <div className="border-border flex min-w-0 flex-1 items-center justify-center overflow-hidden border-r border-l">
        <span className="num text-[20px] leading-none tracking-[0.04em] whitespace-nowrap">
          <Live select={(f) => missionClock(f.t_s)} placeholder="T+00:00:00" />
        </span>
      </div>

      <div className="flex shrink-0 items-stretch">
        <Readout id="altitude" />
        <Rule />
        <Readout id="oat" />
        <Rule />
        <Readout id="ias" />
        <Rule />

        <div className="border-border flex items-center gap-2 border-r px-[14px]">
          {/* Grey, not green. A running engine is the expected case and the
              palette does not spend colour on things being normal. */}
          <span className="bg-muted-foreground block size-[6px] shrink-0" aria-hidden="true" />
          <span className="text-[11px] tracking-[0.1em] whitespace-nowrap">
            <Live select={(f) => f.engine_state} placeholder={NO_VALUE} />
          </span>
        </div>

        <div className="flex items-center gap-2 pr-6 pl-[14px]">
          {/* The dot pulses only while the twin is locked. A still dot and a
              moving one are distinguishable at a glance from across a room,
              which a word in 11px type is not. */}
          <span
            className={`block size-[6px] shrink-0 rounded-full ${
              up ? "bg-muted-foreground" : "bg-structure"
            } ${up && locked ? "animate-lockpulse" : ""}`}
            aria-hidden="true"
          />
          <span className="text-muted-foreground text-[11px] tracking-[0.06em] whitespace-nowrap">
            {up ? (
              <>
                TWIN
                <span className="text-structure mx-[6px]">·</span>
                {/* The number beside the lock is the innovation, not the residual
                    against a healthy engine. It answers whether the twin is
                    tracking the machine in front of it, which stays true while
                    that machine degrades; the residual is the other question and
                    it belongs on the screens that show it channel by channel. */}
                <Live
                  select={(f) =>
                    f.twin
                      ? f.twin.locked
                        ? `LOCKED  ${fmt(f.twin.innovation_pct, 2)}%`
                        : "SYNCING"
                      : "NOT LOCKED"
                  }
                  placeholder="NOT LOCKED"
                />
              </>
            ) : (
              <>
                CAN LINK DOWN
                <span className="text-structure mx-[6px]">·</span>
                {socket === "open" ? "core up" : "core unreachable"}
              </>
            )}
          </span>
        </div>
      </div>
    </header>
  );
}
