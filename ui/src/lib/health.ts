/**
 * Link state, polled rather than inferred.
 *
 * A core whose CAN interface never came up holds an open WebSocket and sends
 * nothing at all, because a frame of zeros would read as "engine stopped" rather
 * than "no data". Socket silence therefore cannot distinguish a dead bus from a
 * quiet moment, and the only honest source for the link is the health endpoint.
 */

export interface Health {
  version: string;
  iface: string;
  clients: number;
  last_seq: number;
  link_ok: boolean;
  twin_locked: boolean;
}

/** How often to ask. Slow: this is a status light, not telemetry. */
const POLL_MS = 2000;

/** Give up on a poll well inside the interval, so requests cannot pile up. */
const TIMEOUT_MS = 1500;

/**
 * Poll `/api/health` until the returned function is called.
 *
 * `onHealth` receives null when the endpoint is unreachable, which is a different
 * failure from `link_ok: false`: one means the core is down, the other means the
 * core is up and the bus is not.
 */
export function pollHealth(onHealth: (health: Health | null) => void): () => void {
  let stopped = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const run = async () => {
    if (stopped) return;
    try {
      const response = await fetch("/api/health", {
        signal: AbortSignal.timeout(TIMEOUT_MS),
      });
      onHealth(response.ok ? ((await response.json()) as Health) : null);
    } catch {
      onHealth(null);
    }
    if (!stopped) timer = setTimeout(run, POLL_MS);
  };

  void run();

  return () => {
    stopped = true;
    if (timer !== undefined) clearTimeout(timer);
  };
}
