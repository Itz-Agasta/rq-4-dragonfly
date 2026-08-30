/**
 * Cold application state.
 *
 * Everything here changes at human rates: which screen is showing, what the
 * operator has acknowledged, what the link is doing. Telemetry is deliberately
 * absent; it lives in `@/store/telemetry` and is read from the render loop.
 */

import { create } from "zustand";

import type { Health } from "@/lib/health";
import type { LinkState } from "@/lib/telemetry";

export const SCREENS = ["ops", "twin", "analysis", "simulate", "replay", "fleet"] as const;

export type ScreenId = (typeof SCREENS)[number];

export type Severity = "caution" | "advisory";

export interface Alert {
  id: string;
  /** Mission time of the event, seconds. */
  t_s: number;
  severity: Severity;
  /** Owning subsystem, as it appears in the health rail. */
  subsystem: string;
  message: string;
  /** Screen that can explain it. Drives the rail's unacknowledged marker. */
  screen: ScreenId;
}

/**
 * What a navigation carries with it.
 *
 * Screens are drill-downs rather than tabs: arriving at one always applies the
 * selection that got you there, so an operator follows the fault instead of
 * navigating to it. Every screen reads this on mount and none renders a default
 * empty state.
 */
export interface Selection {
  /** Channel id, e.g. `egt3`. */
  channel: string | null;
  /** Fault hypothesis id. */
  hypothesis: string | null;
  /** Mission time to scrub to, seconds. */
  t_s: number | null;
}

interface AppState {
  /** Last successful health response, or null if the core is unreachable. */
  health: Health | null;
  /** WebSocket state, which is about the socket rather than about the bus. */
  socket: LinkState;
  alerts: Alert[];
  acked: ReadonlySet<string>;
  selection: Selection;

  /** Draw the twin's predicted trace. Off until there is a twin to draw. */
  showPredicted: boolean;
  /** Dot grid behind the engine schematic. */
  showDotGrid: boolean;

  setHealth: (health: Health | null) => void;
  setSocket: (socket: LinkState) => void;
  setAlerts: (alerts: Alert[]) => void;
  acknowledge: (id: string) => void;
  select: (selection: Partial<Selection>) => void;
  toggle: (key: "showPredicted" | "showDotGrid") => void;
}

export const useApp = create<AppState>((set) => ({
  health: null,
  socket: "connecting",
  alerts: [],
  acked: new Set<string>(),
  selection: { channel: null, hypothesis: null, t_s: null },
  showPredicted: false,
  showDotGrid: true,

  setHealth: (health) => set({ health }),
  setSocket: (socket) => set({ socket }),
  setAlerts: (alerts) => set({ alerts }),
  acknowledge: (id) => set((s) => ({ acked: new Set(s.acked).add(id) })),
  select: (selection) => set((s) => ({ selection: { ...s.selection, ...selection } })),
  toggle: (key) => set((s) => ({ [key]: !s[key] }) as Pick<AppState, typeof key>),
}));

/**
 * Whether a screen carries an unacknowledged alert.
 *
 * Drives the one accent mark the navigation rail is allowed, which is still the
 * alarm meaning rather than a second one.
 *
 * Returns a boolean rather than the set of such screens on purpose. A selector
 * that builds a collection returns a fresh reference every call, so the store
 * sees a change on every read and re-renders without end. Subscribing to the
 * derived boolean is both correct and narrower: a cell re-renders only when its
 * own alert state flips.
 */
export function screenHasUnacknowledged(screen: ScreenId) {
  return (state: AppState): boolean =>
    state.alerts.some((a) => a.screen === screen && !state.acked.has(a.id));
}

/**
 * Whether the engine is being heard from.
 *
 * Both halves matter and they fail differently: an unreachable core means the
 * daemon is down, `link_ok: false` means the daemon is up and the bus is silent.
 */
export function linkIsUp(state: AppState): boolean {
  return state.health?.link_ok === true;
}
