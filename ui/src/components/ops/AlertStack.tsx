/**
 * The alert stack.
 *
 * Entries are derived from the telemetry in `store/events`, not authored, and
 * acknowledgement is real state held in the app store because the navigation
 * rail reads it to decide whether a screen carries an unacknowledged alert.
 *
 * The list is pushed into React state only when the log's version changes, which
 * is at human rates: an event is raised when a detector fires or a residual
 * holds outside its band for two seconds, not on every frame.
 */

import { useRef } from "react";
import { useNavigate } from "react-router";

import { Button } from "@/components/ui/button";
import { missionClock } from "@/lib/fmt";
import { useLiveSink } from "@/lib/live";
import { type Alert, useApp } from "@/store/app";
import { telemetry } from "@/store/telemetry";

function Row({ alert, acked }: { alert: Alert; acked: boolean }) {
  const acknowledge = useApp((s) => s.acknowledge);
  const select = useApp((s) => s.select);
  const navigate = useNavigate();
  const live = alert.severity === "caution" && !acked;

  return (
    <div className="border-border relative border-b py-[10px] pr-4 pl-[14px] last:border-b-0">
      {live ? (
        <span className="bg-primary absolute top-0 bottom-0 left-0 w-[2px]" aria-hidden="true" />
      ) : null}
      <div className="flex items-baseline justify-between gap-[10px]">
        <span className={`text-[11px] ${live ? "text-muted-foreground" : "text-foreground-dim"}`}>
          {missionClock(alert.t_s)}
        </span>
        <span
          className={`text-[11px] tracking-[0.1em] ${live ? "text-primary" : "text-foreground-dim"}`}
        >
          {alert.severity === "caution" ? "CAUTION" : "ADVISORY"} · {acked ? "ACK" : "UNACK"}
        </span>
      </div>
      <div
        className={`mt-[5px] text-[12px] tracking-[0.04em] ${
          live ? "text-primary" : "text-muted-foreground"
        }`}
      >
        {alert.subsystem}
      </div>
      <div
        className={`mt-1 text-[13px] leading-[1.45] text-pretty ${
          live ? "text-foreground" : "text-foreground-dim"
        }`}
      >
        {alert.message}
      </div>
      {live ? (
        <div className="mt-2 flex gap-2">
          <Button size="sm" onClick={() => acknowledge(alert.id)}>
            ACK
          </Button>
          <Button
            size="sm"
            variant="ghost"
            // Screens are drill-downs: this arrives at ANALYSIS with the
            // hypothesis already open rather than at a default empty state.
            // Screens are drill-downs, and the selection is the event's own
            // channel rather than a constant: an alert about LAMBDA 3 that
            // opened TWIN on EGT 3 would be worse than no link at all.
            onClick={() => {
              select({ channel: alert.subsystem, t_s: alert.t_s });
              void navigate(`/${alert.screen}`);
            }}
          >
            EXPLAIN
          </Button>
        </div>
      ) : null}
    </div>
  );
}

export function AlertStack() {
  const setAlerts = useApp((s) => s.setAlerts);
  const acked = useApp((s) => s.acked);
  const alerts = useApp((s) => s.alerts);
  const seen = useRef(-1);

  useLiveSink(() => {
    const log = telemetry.events;
    if (log.version === seen.current) return;
    seen.current = log.version;
    setAlerts(log.list());
  });

  const unacked = alerts.filter((a) => !acked.has(a.id)).length;

  return (
    <section className="border-border flex min-h-0 flex-1 flex-col overflow-hidden border-b">
      <div className="border-border flex h-9 shrink-0 items-center justify-between border-b px-4">
        <span className="t-section">ALERTS</span>
        <span className="label-micro">
          {alerts.length} · {unacked} UNACK
        </span>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {alerts.length === 0 ? (
          // Not "no alerts": on a healthy engine this is the normal state and it
          // is a finding, since the whole claim is that a fault is caught here
          // before anything conventional fires.
          <div className="label-micro p-4">Nothing raised this mission</div>
        ) : (
          alerts.map((a) => <Row key={a.id} alert={a} acked={acked.has(a.id)} />)
        )}
      </div>
    </section>
  );
}
