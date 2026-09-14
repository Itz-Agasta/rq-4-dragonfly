/**
 * The application shell: rail, mission bar, screen.
 *
 * Also the single place the telemetry socket and the health poll are opened. Both
 * live for the lifetime of the app rather than per screen, so switching screens
 * never drops the feed or restarts the ring buffers.
 */

import { useEffect } from "react";
import { Outlet, useLocation, useNavigate } from "react-router";

import { NavRail } from "@/components/app/NavRail";
import { Toasts } from "@/components/app/Toasts";
import { TopBar } from "@/components/app/TopBar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { pollHealth } from "@/lib/health";
import { report } from "@/lib/report";
import { connect } from "@/lib/telemetry";
import { startTourOnFirstVisit } from "@/lib/tour";
import { type ScreenId, useApp } from "@/store/app";
import { telemetry } from "@/store/telemetry";

const TITLES: Record<string, string> = {
  ops: "OPS",
  twin: "TWIN",
  analysis: "ANALYSIS",
  simulate: "SIMULATE",
  replay: "REPLAY",
  fleet: "FLEET",
  evidence: "EVIDENCE",
};

export function Shell() {
  const location = useLocation();
  const navigate = useNavigate();
  const screen = (location.pathname.replace("/", "") || "ops") as ScreenId;

  // Offered once per browser, and never under `just kiosk`. Separate from the
  // socket effect below and deliberately not waiting on it: the tour's first
  // four steps are shell chrome, which renders before a frame arrives, and
  // gating on the feed would leave a new user looking at an unexplained screen
  // for as long as the core takes to come up.
  useEffect(() => startTourOnFirstVisit((to) => void navigate(to)), [navigate]);

  useEffect(() => {
    const { setSocket, setHealth } = useApp.getState();
    const connection = connect({
      onFrame: (frame) => telemetry.push(frame),
      onState: setSocket,
      // Without this a throw anywhere in the store is swallowed by the socket's
      // own guard and the whole app sits at its placeholders looking like a dead
      // feed. `connect` documents the hazard and nothing was listening.
      onDecodeError: (error) => report("frame dropped", error),
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
        <NavRail />
        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <TopBar screen={TITLES[screen] ?? "OPS"} />
          <main className="flex min-h-0 min-w-0 flex-1 flex-col">
            <Outlet />
          </main>
        </div>
        <Toasts />
      </div>
    </TooltipProvider>
  );
}
