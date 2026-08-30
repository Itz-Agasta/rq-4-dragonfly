/**
 * The application shell: rail, mission bar, screen.
 *
 * Also the single place the telemetry socket and the health poll are opened. Both
 * live for the lifetime of the app rather than per screen, so switching screens
 * never drops the feed or restarts the ring buffers.
 */

import { useEffect } from "react";
import { Outlet, useLocation } from "react-router";

import { NavRail } from "@/components/app/NavRail";
import { TopBar } from "@/components/app/TopBar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { pollHealth } from "@/lib/health";
import { connect } from "@/lib/telemetry";
import { type ScreenId, useApp } from "@/store/app";
import { telemetry } from "@/store/telemetry";

const TITLES: Record<string, string> = {
  ops: "OPS",
  twin: "TWIN",
  analysis: "ANALYSIS",
  simulate: "SIMULATE",
  replay: "REPLAY",
  fleet: "FLEET",
};

/** `⌐ ¬` at the four viewport corners. Cheap, and most of what reads as designed. */
function Brackets() {
  const base = "text-structure-hi pointer-events-none absolute z-10 text-[12px] leading-none";
  return (
    <div aria-hidden="true">
      <span className={`${base} top-[5px] left-[7px]`}>⌐</span>
      <span className={`${base} top-[5px] right-[7px]`}>¬</span>
      <span className={`${base} bottom-[5px] left-[7px] -scale-y-100`}>⌐</span>
      <span className={`${base} right-[7px] bottom-[5px] -scale-y-100`}>¬</span>
    </div>
  );
}

export function Shell() {
  const location = useLocation();
  const screen = (location.pathname.replace("/", "") || "ops") as ScreenId;

  useEffect(() => {
    const { setSocket, setHealth } = useApp.getState();
    const connection = connect({
      onFrame: (frame) => telemetry.push(frame),
      onState: setSocket,
    });
    const stopPolling = pollHealth(setHealth);
    return () => {
      connection.close();
      stopPolling();
    };
  }, []);

  return (
    <TooltipProvider delayDuration={120}>
      <div className="bg-background text-foreground relative flex h-full w-full overflow-hidden">
        <Brackets />
        <NavRail />
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <TopBar screen={TITLES[screen] ?? "OPS"} />
          <main className="flex min-h-0 min-w-0 flex-1 flex-col">
            <Outlet />
          </main>
        </div>
      </div>
    </TooltipProvider>
  );
}
