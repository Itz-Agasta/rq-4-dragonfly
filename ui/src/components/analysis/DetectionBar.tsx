/**
 * The two detectors and the monitor they replace, side by side.
 *
 * This strip is the claim the whole build rests on, so it shows the conventional
 * redline alongside the twin's own tests rather than only the twin's verdict. On
 * the demonstration fault the redline reads `no trip` for the entire run, because
 * a coked injector runs its cylinder cool and every certificated limit that could
 * see it is an upper bound. A screen that omitted the comparison would be asking
 * to be taken on trust.
 *
 * While the detector is calibrating, the absence of a drift alarm says nothing
 * about the engine, so it says `calibrating` rather than `nominal`.
 */

import { useRef } from "react";

import { fmt, missionClock, NO_VALUE } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import { isFresh } from "@/lib/telemetry";

// Widest the readout can grow without reflowing the detector strip.
const CUSUM_DISPLAY_MAX = 9999;

export function DetectionBar() {
  const state = useRef<HTMLSpanElement>(null);
  const cusum = useRef<HTMLSpanElement>(null);
  const cusumWhere = useRef<HTMLSpanElement>(null);
  const distance = useRef<HTMLSpanElement>(null);
  const onset = useRef<HTMLSpanElement>(null);
  const redline = useRef<HTMLSpanElement>(null);

  useLiveSink((frame) => {
    const twin = frame.twin;
    const fresh = isFresh(frame.ages.engine_ms);
    const d = twin?.detection;

    if (!d || !fresh) {
      [state, cusum, cusumWhere, distance, onset, redline].forEach((slot) => {
        if (slot.current) slot.current.textContent = NO_VALUE;
      });
      if (state.current) state.current.removeAttribute("data-alarm");
      return;
    }

    if (state.current) {
      state.current.textContent = d.calibrating
        ? "CALIBRATING"
        : d.anomaly
          ? "ANOMALY"
          : d.drift
            ? "DRIFT"
            : "NOMINAL";
      // Calibrating is not an alarm and must not paint like one; it is the
      // detector saying it has no opinion yet.
      if (!d.calibrating && (d.anomaly || d.drift)) {
        state.current.setAttribute("data-alarm", "");
      } else {
        state.current.removeAttribute("data-alarm");
      }
    }

    if (cusum.current) {
      // A CUSUM accumulates without bound while a fault is held, so after an hour
      // on the demonstration fault it runs to six figures. The readout is capped
      // to keep the column width, but a capped value is marked as one: printing
      // a flat `9999.0` states a precision the detector never reported.
      const over = d.cusum > CUSUM_DISPLAY_MAX;
      cusum.current.textContent = d.calibrating
        ? NO_VALUE
        : `${over ? ">" : ""}${fmt(Math.min(d.cusum, CUSUM_DISPLAY_MAX), over ? 0 : 1)} / ${fmt(d.cusum_limit, 0)}`;
    }
    if (cusumWhere.current) {
      cusumWhere.current.textContent = d.cusum_channel || "";
    }
    if (distance.current) {
      distance.current.textContent = `${fmt(d.distance, 2)} / ${fmt(d.distance_limit, 2)}`;
    }
    if (onset.current) {
      onset.current.textContent = d.drift_since === null ? NO_VALUE : missionClock(d.drift_since);
    }
    if (redline.current) {
      redline.current.textContent =
        d.redline_since === null
          ? "no trip"
          : `${d.redline_channel} at ${missionClock(d.redline_since)}`;
    }
  });

  return (
    <div className="border-border bg-card flex shrink-0 items-stretch border-b">
      <Field label="detector">
        <span ref={state} role="status" className="t-small text-foreground">
          {NO_VALUE}
        </span>
      </Field>
      <Field label="cusum">
        <span className="flex items-baseline gap-2">
          <span ref={cusum} className="num t-small text-foreground">
            {NO_VALUE}
          </span>
          <span ref={cusumWhere} className="label-micro" />
        </span>
      </Field>
      <Field label="mahalanobis">
        <span ref={distance} className="num t-small text-foreground">
          {NO_VALUE}
        </span>
      </Field>
      <Field label="drift since">
        <span ref={onset} className="num t-small text-foreground">
          {NO_VALUE}
        </span>
      </Field>
      <Field label="certified redline" grow>
        <span ref={redline} className="t-small text-muted-foreground">
          {NO_VALUE}
        </span>
      </Field>
    </div>
  );
}

function Field({
  label,
  grow,
  children,
}: {
  label: string;
  grow?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div
      className={`border-border flex min-w-0 flex-col justify-center gap-1 border-r px-4 py-2 ${
        grow ? "flex-1" : "shrink-0"
      }`}
    >
      <span className="label-micro">{label}</span>
      {children}
    </div>
  );
}
