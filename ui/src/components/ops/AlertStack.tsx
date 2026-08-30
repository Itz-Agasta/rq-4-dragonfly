/**
 * The alert stack.
 *
 * MOCK: entries come from `./data`. Acknowledgement is real state, held in the
 * app store, because the navigation rail reads it to decide whether a screen
 * carries an unacknowledged alert.
 */

import { useEffect } from "react";
import { useNavigate } from "react-router";

import { ALERTS, PRE_ACKED } from "@/components/ops/data";
import { Button } from "@/components/ui/button";
import { missionClock } from "@/lib/fmt";
import { type Alert, useApp } from "@/store/app";

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
            onClick={() => {
              select({ channel: "egt3", hypothesis: "injector-coking", t_s: alert.t_s });
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
  const acknowledge = useApp((s) => s.acknowledge);
  const acked = useApp((s) => s.acked);
  const alerts = useApp((s) => s.alerts);

  useEffect(() => {
    setAlerts([...ALERTS]);
    for (const id of PRE_ACKED) acknowledge(id);
  }, [setAlerts, acknowledge]);

  const unacked = alerts.filter((a) => !acked.has(a.id)).length;

  return (
    <section className="border-border flex min-h-0 shrink flex-col overflow-hidden border-b">
      <div className="border-border flex h-9 shrink-0 items-center justify-between border-b px-4">
        <span className="t-section">ALERTS</span>
        <span className="label-micro">
          {alerts.length} · {unacked} UNACK
        </span>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {alerts.length === 0 ? (
          <div className="label-micro p-4">No active alerts</div>
        ) : (
          alerts.map((a) => <Row key={a.id} alert={a} acked={acked.has(a.id)} />)
        )}
      </div>
    </section>
  );
}
