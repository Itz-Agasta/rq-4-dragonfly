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
import { Skeleton } from "@/components/ui/skeleton";
import { setFrameSource } from "@/lib/live";
import { listMissions } from "@/lib/mission";
import { report } from "@/lib/report";
import { session, useReplay } from "@/store/replay";

export function Replay() {
  const status = useReplay((s) => s.status);
  const error = useReplay((s) => s.error);
  const open = useReplay((s) => s.open);

  // Every readout in the app, the mission bar included, follows the playhead
  // while a recording is loaded. Restored on the way out or the whole app
  // freezes at the last recorded frame.
  //
  // Gated on `ready`, not on mount: the render loop skips its sinks when the
  // source has no frame, so installing one that returns null leaves the mission
  // bar holding the live values it last painted, with no staleness to dim them.
  // Arriving from OPS that is 300 ms of frozen telemetry reading as current, and
  // on a core with no recordings it is indefinite.
  useEffect(() => {
    if (status !== "ready") return;
    setFrameSource(() => session.frame());
    return () => {
      setFrameSource(null);
      session.stop();
    };
  }, [status]);

  useEffect(() => {
    let cancelled = false;
    listMissions()
      .then((missions) => {
        // The newest *readable* one, which is the one someone just flew. `list`
        // is oldest first, and a recording still being written is not in it at
        // all: parquet keeps its schema in a footer written at close, so the
        // mission being recorded right now cannot be read until the core stops.
        //
        // The frame count is the gate rather than the position. A core stopped
        // before its first flush leaves a closed recording of 16 kB of footer
        // and no rows, and it is the newest file on disk; opening it gives six
        // empty panels with nothing on screen saying why. There are four such
        // files in `data/missions` already.
        const newest = missions.findLast((one) => one.frames > 0);
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

  if (status === "error") {
    return (
      <div className="flex min-h-0 flex-1 items-center justify-center">
        <div className="max-w-[420px] text-center">
          <div className="t-section text-muted-foreground">NO MISSION TO REPLAY</div>
          <div className="label-micro mt-2 leading-[1.5] normal-case">
            {error === "no recordings"
              ? "the core records to data/missions; a mission in progress cannot be read until it closes, and one with no frames in it cannot be drawn"
              : error}
          </div>
        </div>
      </div>
    );
  }

  if (status !== "ready") return <Loading />;

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

/**
 * The layout, before the recording is in it.
 *
 * A skeleton, not a message: the overview pass lands in about a second and a
 * message that brief reads as a flash. The blocks sit on the real panel geometry
 * so nothing moves when the frames arrive.
 *
 * SIMULATE answers the same question the other way, and should: seven seconds is
 * long enough to owe the room a reason and an elapsed count.
 */
function Loading() {
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col" aria-busy="true">
      <div className="border-border flex h-[90px] min-w-0 shrink-0 items-stretch border-b">
        <div className="border-border flex shrink-0 items-center gap-[14px] border-r px-[18px]">
          <Skeleton className="h-7 w-[108px]" />
          <Skeleton className="h-7 w-[196px]" />
        </div>
        <div className="min-w-0 flex-1 px-[18px] py-3">
          <Skeleton className="h-full w-full" />
        </div>
      </div>

      <div className="flex min-h-0 min-w-0 flex-1 items-stretch">
        <div className="border-border flex w-[260px] shrink-0 flex-col border-r">
          <PanelHead label="MISSION PROFILE" />
          <div className="flex min-h-0 flex-1 flex-col gap-4 p-4">
            <Skeleton className="min-h-0 flex-1" />
            <Skeleton className="min-h-0 flex-1" />
            <Skeleton className="h-[104px] shrink-0" />
          </div>
        </div>

        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          <PanelHead label="TELEMETRY · FULL MISSION" />
          {[0, 1, 2].map((row) => (
            <div
              key={row}
              className="border-border flex min-h-0 min-w-0 flex-1 items-stretch border-b last:border-b-0"
            >
              {[0, 1].map((cell) => (
                <div
                  key={cell}
                  className="border-border flex min-h-0 min-w-0 flex-1 flex-col border-r px-4 py-[10px] last:border-r-0"
                >
                  <div className="flex items-center justify-between gap-[10px]">
                    <Skeleton className="h-2 w-20" />
                    <Skeleton className="h-3 w-14" />
                  </div>
                  <Skeleton className="mt-[6px] min-h-0 flex-1" />
                </div>
              ))}
            </div>
          ))}
        </div>

        <div className="border-border flex w-[300px] shrink-0 flex-col border-l">
          <PanelHead label="MISSION EVENTS" />
          <div className="flex min-h-0 flex-1 flex-col gap-3 p-4">
            {[0, 1, 2, 3, 4, 5].map((row) => (
              <Skeleton key={row} className="h-9 shrink-0" />
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

/** A panel header that is already true while the panel under it is not. */
function PanelHead({ label }: { label: string }) {
  return (
    <div className="border-border flex h-9 shrink-0 items-center border-b px-4">
      <span className="t-section text-muted-foreground">{label}</span>
    </div>
  );
}
