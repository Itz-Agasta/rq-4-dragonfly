/**
 * The wire contract with dragonfly-core.
 *
 * Mirrors the `Frame` struct in `crates/dragonfly-core/src/telemetry.rs`. The two
 * are kept in step by hand: there are fifty fields and one struct, and a code
 * generator for that is more moving parts than the thing it generates. If a
 * field appears here that the core does not send it arrives as `undefined`, so
 * add fields to the Rust side first.
 *
 * Frames are MessagePack, not JSON. At 20 Hz with fifty fields, JSON spends most
 * of its bytes repeating field names and the browser spends real time parsing
 * them.
 */

import { decode } from "@msgpack/msgpack";

/** Number of cylinders. Fixed by the engine, not by configuration. */
export const CYLINDERS = 4;

/** How long ago each source last spoke, in milliseconds. */
export interface SourceAges {
  /** Engine controller, node 42. */
  engine_ms: number;
  /** Vendor auxiliary message, node 42. */
  auxiliary_ms: number;
  /** Fuel tank status, node 42. */
  fuel_ms: number;
  /** Air data computer, node 43. */
  air_data_ms: number;
  /** Power module, node 44. */
  power_ms: number;
}

/** Engine state as the controller reports it. */
export type EngineState = "STOPPED" | "STARTING" | "RUNNING" | "FAULT";

/**
 * One instant of measured telemetry.
 *
 * Everything here is a measurement or is derived from measurements alone.
 * Nothing is predicted: the twin's output is a separate frame, and keeping them
 * apart at the transport is what makes provenance impossible to get wrong
 * further up.
 */
export interface Frame {
  /** Monotonic sequence number. A gap means this client fell behind. */
  seq: number;
  /** Seconds since ingest started. */
  t_s: number;
  /** False when the engine has not been heard from for 250 ms. */
  link_ok: boolean;
  /** Age of each source. */
  ages: SourceAges;

  /** Pressure altitude, m. */
  altitude_m: number;
  /** Outside air temperature, K. */
  oat_k: number;
  /** Ambient static pressure, Pa. */
  p_amb_pa: number;
  /** Indicated airspeed, m/s. */
  ias_ms: number;
  /** Deviation from the standard atmosphere, K. */
  isa_deviation_k: number;
  /** Fuelling demand, percent. */
  throttle_pct: number;
  /** Brake load, percent of rating. */
  load_pct: number;

  /** Crankshaft speed, rpm. */
  rpm: number;
  /** Intake manifold absolute pressure, Pa. */
  map_pa: number;
  /** Intake manifold temperature, K. */
  mat_k: number;
  /** Boost, Pa. Manifold pressure less ambient. */
  boost_pa: number;
  /** Air mass flow, kg/s. */
  maf_kgs: number;
  /** Fuel flow, kg/h. */
  fuel_flow_kgh: number;
  /** Fuel flow, litres/hour. */
  fuel_flow_lph: number;
  /** Mean excess air ratio. */
  lambda: number;
  /** Excess air ratio per cylinder. */
  lambda_k: number[];
  /** Cylinder head temperature per cylinder, K. */
  cht_k: number[];
  /** Exhaust gas temperature per cylinder, K. */
  egt_k: number[];
  /** Injection duration per cylinder, ms. */
  injection_ms: number[];
  /** Oil gallery pressure, Pa. */
  oil_p_pa: number;
  /** Oil temperature, K. */
  oil_t_k: number;
  /** Coolant temperature, K. */
  coolant_t_k: number;
  /** Turbocharger shaft speed, rpm. */
  tc_rpm: number;
  /** Bus voltage, V. */
  bus_v: number;
  /** Broadband vibration, g RMS. */
  vib_rms_g: number;
  /** Kurtosis of the vibration signal. */
  vib_kurtosis: number;

  /** Wastegate position, 0 shut to 1 open. */
  wastegate: number;
  /** Fuel remaining, percent. */
  fuel_remaining_pct: number;
  /** Engine state. */
  engine_state: EngineState;
  /** Raw DroneCAN status flag bitmask. */
  flags: number;
}

/** Connection state, for the shell to render a link indicator. */
export type LinkState = "connecting" | "open" | "closed";

/** What {@link connect} gives back. */
export interface Connection {
  /** Close the socket and stop reconnecting. */
  close(): void;
}

/** Options for {@link connect}. */
export interface ConnectOptions {
  /** WebSocket URL. Defaults to `/ws` on the current origin. */
  url?: string;
  /** Called for every frame. */
  onFrame: (frame: Frame) => void;
  /** Called when the connection state changes. */
  onState?: (state: LinkState) => void;
  /**
   * Called when a frame fails to decode.
   *
   * Reported rather than logged so a decode bug surfaces in the app's own error
   * surface instead of only in a console nobody has open during a demo.
   */
  onDecodeError?: (error: unknown) => void;
}

const RECONNECT_MIN_MS = 250;
const RECONNECT_MAX_MS = 5000;

/**
 * Open the telemetry socket and keep it open.
 *
 * Reconnects with a backoff, because the core is restarted constantly during
 * development and a page that has to be reloaded to pick it up again wastes more
 * time than this costs.
 *
 * The callback runs at 20 Hz. Write what it gives you into a store read inside
 * the render loop; putting it into React state re-renders the tree twenty times
 * a second and the frame budget goes to reconciliation.
 */
export function connect(options: ConnectOptions): Connection {
  const url = options.url ?? defaultUrl();
  let socket: WebSocket | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let backoff = RECONNECT_MIN_MS;
  let closed = false;

  const open = () => {
    if (closed) return;
    options.onState?.("connecting");
    const ws = new WebSocket(url);
    socket = ws;
    ws.binaryType = "arraybuffer";

    ws.addEventListener("open", () => {
      backoff = RECONNECT_MIN_MS;
      options.onState?.("open");
    });

    ws.addEventListener("message", (event: MessageEvent<ArrayBuffer>) => {
      // A malformed frame must not kill the socket: the next one is 50 ms away
      // and dropping the connection turns a decode bug into a link outage.
      try {
        options.onFrame(decode(new Uint8Array(event.data)) as Frame);
      } catch (error) {
        options.onDecodeError?.(error);
      }
    });

    ws.addEventListener("close", () => {
      options.onState?.("closed");
      if (closed) return;
      timer = setTimeout(open, backoff);
      backoff = Math.min(backoff * 2, RECONNECT_MAX_MS);
    });

    ws.addEventListener("error", () => ws.close());
  };

  open();

  return {
    close() {
      closed = true;
      if (timer !== null) clearTimeout(timer);
      socket?.close();
    },
  };
}

function defaultUrl(): string {
  const protocol = location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${location.host}/ws`;
}

/**
 * Whether a channel's source is fresh enough to display as live.
 *
 * A frozen trace looks exactly like a steady one, so anything rendering a value
 * has to consult this. It is the same 250 ms the core uses for its own link flag.
 */
export function isFresh(ageMs: number): boolean {
  return ageMs < 250;
}
