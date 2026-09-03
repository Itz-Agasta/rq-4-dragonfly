/**
 * The recorded-mission API.
 *
 * A recording is served as the same `Frame` the WebSocket carries, in the same
 * MessagePack encoding, so everything downstream of a frame works on a replay
 * without knowing it is one. Three things are absent from a recorded frame and
 * each is deliberate: no prognosis, no isolation detail, no source ages. Nothing
 * on REPLAY may draw a remaining life, and a replayed frame must never be routed
 * to ANALYSIS.
 */

import { decode } from "@msgpack/msgpack";

import type { Frame } from "@/lib/telemetry";

/** One recording on the daemon's disk. Mirrors `replay::MissionInfo`. */
export interface MissionInfo {
  /** File stem, which is what the API addresses a mission by. */
  id: string;
  frames: number;
  /** Mission time of the last frame, seconds. */
  duration_s: number;
  bytes: number;
}

/** Which part of a recording to ask for. Mirrors `replay::Window`. */
export interface MissionWindow {
  /** Keep one frame in `stride`. */
  stride?: number;
  /** First frame, counted before striding. */
  from?: number;
  /** Frames after striding. The daemon clamps this at 20,000. */
  count?: number;
}

/**
 * Frames the overview pass keeps, one in this many.
 *
 * 0.2 Hz, 2,880 frames on a four hour mission: measured at 6.9 MB in 0.26 s
 * against 34.7 MB at 1 Hz, and already three points per pixel on a timeline a
 * thousand pixels wide. The rules in `store/events.ts` are written against
 * mission time rather than frame counts, so their sustain and clear windows
 * still mean seconds at this sampling.
 */
export const OVERVIEW_STRIDE = 100;

/** Every recording the daemon can serve, oldest first. */
export async function listMissions(): Promise<MissionInfo[]> {
  const response = await fetch("/api/missions");
  if (!response.ok) throw new Error(`listing missions: ${response.status}`);
  return (await response.json()) as MissionInfo[];
}

/**
 * Read part of a recording.
 *
 * The window is pushed into the daemon rather than applied here, so a window
 * late in a long mission does not decompress the hours before it.
 */
export async function readMission(id: string, window: MissionWindow): Promise<Frame[]> {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(window)) {
    if (value !== undefined) query.set(key, String(value));
  }
  const response = await fetch(`/api/missions/${id}?${query.toString()}`);
  if (!response.ok) throw new Error(`reading ${id}: ${response.status}`);
  return decode(await response.arrayBuffer()) as Frame[];
}

/**
 * Bytes as an operator reads them.
 *
 * Computed from the directory listing rather than authored: a recorder that
 * fits four hours in 96 MB is an argument for carrying one, and a figure typed
 * in is the kind a reader divides in their head.
 */
export function bytesLabel(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${Math.round(bytes / 1e6)} MB`;
  return `${Math.round(bytes / 1e3)} kB`;
}

/** Publish rate the recording was made at, from its own frame count. */
export function rateHz(info: MissionInfo): number {
  return info.duration_s > 0 ? Math.round(info.frames / info.duration_s) : 0;
}
