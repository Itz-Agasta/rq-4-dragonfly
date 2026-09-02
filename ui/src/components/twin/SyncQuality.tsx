/**
 * How well the twin is tracking, and where it is not.
 *
 * The hero is the residual RMS across all twenty-two channels, which is the one
 * number that says whether anything else on this screen can be believed. The
 * table beneath it is per channel, and it reports the residual's **mean** and
 * **standard deviation** over the retained window rather than the measurement's.
 *
 * That distinction is the panel. A mean residual is the twin's standing bias on
 * a channel and it is not zero: model-plant mismatch leaves an offset, measured
 * at -0.69 sigma on brake torque, and it is the reason the CUSUM in `twin-core`
 * has to baseline the engine before it will accumulate anything. A panel titled
 * TWIN SYNC QUALITY that showed the engine's own mean and spread instead would
 * be a statistics table about the machine, with the twin's agreement squeezed
 * into its last column.
 *
 * Rows are in measurement vector order and never sort. The rail beside this panel
 * is the ranked view of the same numbers; having both sorted would leave nowhere
 * to look up a channel by name, and a table that reorders under a reader is worse
 * than one they have to scan.
 */

import { useRef } from "react";

import { fmt, missionClock, NO_VALUE, signed } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import { isFresh } from "@/lib/telemetry";
import { COMPARED } from "@/store/compared";
import { HISTORY_SECONDS, telemetry } from "@/store/telemetry";
import { BAND_SIGMA } from "@/store/twin";

const COLUMNS = "grid grid-cols-[1fr_46px_38px_46px] gap-x-[6px]";

export function SyncQuality() {
  const rms = useRef<HTMLSpanElement>(null);
  const lock = useRef<HTMLSpanElement>(null);
  const state = useRef<HTMLSpanElement>(null);
  const means = useRef<(HTMLSpanElement | null)[]>([]);
  const sds = useRef<(HTMLSpanElement | null)[]>([]);
  const nows = useRef<(HTMLSpanElement | null)[]>([]);

  /**
   * Mission time the twin **acquired** lock, not the last frame it held it.
   *
   * Written on the false-to-true edge only. Written every locked frame it would
   * track the mission clock in the top bar and say nothing at all.
   */
  const lockedAt = useRef<number | null>(null);
  const wasLocked = useRef(false);

  useLiveSink((frame) => {
    const twin = frame.twin;
    const live = twin !== null && isFresh(frame.ages.engine_ms);

    const locked = twin?.locked === true;
    if (locked && !wasLocked.current) lockedAt.current = frame.t_s;
    if (!locked) lockedAt.current = null;
    wasLocked.current = locked;
    if (rms.current) rms.current.textContent = live ? fmt(twin.rms_pct, 2) : NO_VALUE;
    if (state.current) {
      state.current.textContent = !live ? "NO TWIN" : twin.locked ? "LOCKED" : "CONVERGING";
      state.current.toggleAttribute("data-alarm", live && !twin.locked);
    }
    if (lock.current) {
      lock.current.textContent =
        lockedAt.current === null ? NO_VALUE : missionClock(lockedAt.current);
    }

    const history = telemetry.twin;
    for (let i = 0; i < COMPARED.length; i += 1) {
      const summary = live ? history.summary(i) : null;
      const mean = means.current[i];
      const sd = sds.current[i];
      const now = nows.current[i];
      if (mean) mean.textContent = summary ? signed(summary.mean, 2) : NO_VALUE;
      if (sd) sd.textContent = summary ? fmt(summary.sd, 2) : NO_VALUE;
      if (now) {
        now.textContent = summary ? signed(summary.now, 2) : NO_VALUE;
        now.toggleAttribute("data-alarm", summary !== null && Math.abs(summary.now) > BAND_SIGMA);
      }
    }
  });

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden">
      <div className="border-border flex h-[36px] shrink-0 items-center justify-between gap-2 border-b px-4">
        <span className="t-section">TWIN SYNC QUALITY</span>
        <span ref={state} className="label-micro shrink-0">
          NO TWIN
        </span>
      </div>

      <div className="border-border flex shrink-0 items-baseline justify-between gap-3 border-b px-4 pt-3 pb-[14px]">
        <div>
          <div className="label-micro">residual RMS · all ch</div>
          <div className="mt-1 flex items-baseline">
            <span ref={rms} className="t-value">
              {NO_VALUE}
            </span>
            <span className="text-muted-foreground ml-1 text-[11px]">%</span>
          </div>
        </div>
        <div className="text-right">
          <div className="label-micro">lock acquired</div>
          <span ref={lock} className="num mt-[5px] block text-[14px]">
            {NO_VALUE}
          </span>
        </div>
      </div>

      <div className={`${COLUMNS} shrink-0 px-4 pt-2 pb-1`}>
        {/* The window is named in the column header rather than left implied: a
            mean over ninety seconds and a mean over the mission are different
            claims, and this one is the former. */}
        <span className="label-micro">ch · {HISTORY_SECONDS} s</span>
        <span className="label-micro text-right">mean</span>
        <span className="label-micro text-right">σ</span>
        <span className="label-micro text-right">now</span>
      </div>

      {/* The rows share the height rather than scrolling. Twenty-two channels
          with half of them below a fold is a panel that has to be operated to be
          read, on a screen that is read from across a room. */}
      <div className="flex min-h-0 flex-1 flex-col px-4 pb-3">
        {COMPARED.map((ch, i) => (
          <div
            key={ch.name}
            className={`${COLUMNS} border-border min-h-[14px] flex-1 items-center border-t`}
          >
            <span className="truncate text-[11px] leading-[1.4]">{ch.name}</span>
            <span
              ref={(el) => {
                means.current[i] = el;
              }}
              className="num text-muted-foreground text-right text-[11px] leading-[1.4]"
            >
              {NO_VALUE}
            </span>
            <span
              ref={(el) => {
                sds.current[i] = el;
              }}
              className="num text-foreground-dim text-right text-[11px] leading-[1.4]"
            >
              {NO_VALUE}
            </span>
            <span
              ref={(el) => {
                nows.current[i] = el;
              }}
              className="num text-right text-[11px] leading-[1.4]"
            >
              {NO_VALUE}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
