/**
 * REPLAY: a recorded mission, scrubbed.
 *
 * A data source swap and nothing more. `replay::read` returns the `Frame` the
 * WebSocket carries, so the channel registry, the event rules and every readout
 * work on a recording without knowing it is one.
 *
 * Two absences in a recorded frame are load bearing here. There is **no
 * prognosis**, a fit over a span the recording does not store, so nothing here
 * draws a remaining life; and **no isolation detail**, which is why a replayed
 * frame is never routed to ANALYSIS.
 */

import { useEffect } from "react";

import { EventLog } from "@/components/replay/EventLog";
import { MissionStrips } from "@/components/replay/MissionStrips";
import { Profile } from "@/components/replay/Profile";
import { Timeline } from "@/components/replay/Timeline";
import { setFrameSource } from "@/lib/live";
import { listMissions } from "@/lib/mission";
import { report } from "@/lib/report";
import { session, useReplay } from "@/store/replay";

export function Replay() {
  const status = useReplay((s) => s.status);
  const error = useReplay((s) => s.error);
  const open = useReplay((s) => s.open);

  // Every readout in the app, the mission bar included, follows the playhead
  // while this screen is mounted. Restored on the way out or the whole app
  // freezes at the last recorded frame.
  useEffect(() => {
    setFrameSource(() => session.frame());
    return () => {
      setFrameSource(null);
      session.stop();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    listMissions()
      .then((missions) => {
        // The newest, which is the one someone just flew. `list` is oldest
        // first, and a recording still being written is not in it at all: parquet
        // keeps its schema in a footer written at close, so the mission being
        // recorded right now cannot be read until the core stops.
        const newest = missions[missions.length - 1];
        if (!cancelled && newest) void open(newest);
        else if (!cancelled) useReplay.setState({ status: "error", error: "no recordings" });
      })
      .catch((cause: unknown) => {
        report("listing missions", cause);
        if (!cancelled) {
          useReplay.setState({
            status: "error",
            error: cause instanceof Error ? cause.message : String(cause),
          });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [open]);

  // Bound once. Reading `playing` through the store rather than closing over it
  // is what keeps this from tearing the listener down and rebuilding it on every
  // transport press.
  useEffect(() => {
    const keys = (event: KeyboardEvent) => {
      if (event.target instanceof HTMLInputElement) return;
      if (event.code === "Space") {
        event.preventDefault();
        const { playing, setPlaying } = useReplay.getState();
        setPlaying(!playing);
      } else if (event.code === "ArrowLeft") {
        session.step(-1);
      } else if (event.code === "ArrowRight") {
        session.step(1);
      }
    };
    window.addEventListener("keydown", keys);
    return () => window.removeEventListener("keydown", keys);
  }, []);

  if (status !== "ready") {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center">
        <div className="text-center">
          <div className="t-section text-muted-foreground">
            {status === "error" ? "NO MISSION TO REPLAY" : "READING RECORDING"}
          </div>
          <div className="label-micro mt-2">
            {status === "error"
              ? error === "no recordings"
                ? "the core records to data/missions; a mission in progress cannot be read until it closes"
                : error
              : "decoding the overview pass"}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <Timeline />
      <div className="flex min-h-0 min-w-0 flex-1 items-stretch">
        <Profile />
        <MissionStrips />
        <EventLog />
      </div>
    </div>
  );
}
