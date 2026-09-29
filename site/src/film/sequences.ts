// Frame sequences cut from site/gen/masters by site/pipeline/frames.sh.
// Counts must match what that script wrote; a wrong count requests 404s past the end.
export const SEQUENCES = {
  S1: 120,
  S2: 120,
  S3: 99,
  S4: 150,
  S9: 120,
  S10: 99,
} as const;

export type SeqId = keyof typeof SEQUENCES;

/**
 * Where frames and card footage are served from. Empty means this origin (the dev
 * server's public/); production points it at the R2 bucket, which holds the same
 * frames/ and media/ layout. Set VITE_ASSETS_BASE at build time.
 */
export const ASSETS = ((import.meta.env.VITE_ASSETS_BASE as string | undefined) ?? "").replace(
  /\/$/,
  "",
);
const BASE = `${ASSETS}/frames`;

// 960w frames below 900 css px: a phone never shows the 1920w detail and the
// mobile set is a third of the bytes.
const variant = () => (Math.min(innerWidth, innerHeight * (16 / 9)) < 900 ? "m" : "d");

const url = (id: SeqId, v: "m" | "d", i: number) =>
  `${BASE}/${id}/${v}/${String(i + 1).padStart(4, "0")}.avif`;

// Two tiers, armory's approach. Every sequence loads its 960w set first (a third of
// the bytes), and desktops then stream the 1920w set over it. The loader gates on the
// low tier only: on 4G the hero's 1920w set alone (~16 MB) outlasts any sane wait.
type Seq = {
  lo: (HTMLImageElement | null)[];
  hi: (HTMLImageElement | null)[];
  started: boolean;
  loadedLo: number;
  settled: number; // every task, either tier, succeeded or failed
};
const store = new Map<SeqId, Seq>();
let decoded: (id: SeqId) => void = () => {};

/** The film canvas registers here to repaint when a better frame for its sequence lands. */
export const onDecoded = (cb: (id: SeqId) => void) => (decoded = cb);

function seq(id: SeqId): Seq {
  let s = store.get(id);
  if (!s) {
    const n = SEQUENCES[id];
    s = {
      lo: Array(n).fill(null),
      hi: Array(n).fill(null),
      started: false,
      loadedLo: 0,
      settled: 0,
    };
    store.set(id, s);
  }
  return s;
}

// Coarse to fine: every 8th frame, then every 4th, 2nd, then the rest. A reader who
// scrubs before the set finishes sees a lower frame rate, never a blank canvas.
function order(n: number): number[] {
  const seen = new Set<number>();
  const out: number[] = [];
  for (const stride of [8, 4, 2, 1]) {
    for (let i = 0; i < n; i += stride) {
      if (seen.has(i)) continue;
      seen.add(i);
      out.push(i);
    }
  }
  return out;
}

/** Starts loading a sequence once; resolves when its low tier's coarse pass is in. */
export function load(id: SeqId): Promise<void> {
  const s = seq(id);
  const n = SEQUENCES[id];
  if (s.started) return Promise.resolve();
  s.started = true;
  const tasks: ["lo" | "hi", number][] = order(n).map((i) => ["lo", i]);
  if (variant() === "d") tasks.push(...order(n).map((i) => ["hi", i] as ["hi", number]));
  const coarse = Math.ceil(n / 8);
  let resolveCoarse!: () => void;
  const coarseDone = new Promise<void>((r) => (resolveCoarse = r));
  let next = 0;
  // Six in flight: enough to saturate a CDN connection, few enough that the coarse
  // pass of the scene the reader is on is not queued behind the fine pass.
  const pump = () => {
    if (next >= tasks.length) return;
    const [tier, i] = tasks[next++];
    const img = new Image();
    img.decoding = "async";
    img.src = url(id, tier === "lo" ? "m" : "d", i);
    img
      .decode()
      .then(() => {
        s[tier][i] = img;
        decoded(id);
      })
      .catch(() => {})
      .finally(() => {
        s.settled++;
        if (tier === "lo" && ++s.loadedLo === coarse) resolveCoarse();
        pump();
      });
  };
  for (let k = 0; k < 6; k++) pump();
  return coarseDone;
}

/** Best frame at or nearest to `i`: the sharp tier if it has landed, else the light one. */
export function nearest(id: SeqId, i: number): HTMLImageElement | null {
  const { lo, hi } = seq(id);
  for (let d = 0; d < lo.length; d++) {
    const a = hi[i - d] ?? lo[i - d];
    if (a) return a;
    const b = hi[i + d] ?? lo[i + d];
    if (b) return b;
  }
  return null;
}

/** Fraction of a sequence's low tier decoded, 0..1. Gates the loader, nothing else. */
export const progress = (id: SeqId) => seq(id).loadedLo / SEQUENCES[id];

/** Every frame of every tier this device loads has settled; `?record` waits on it. */
export const complete = (id: SeqId) => {
  const s = seq(id);
  return s.settled >= SEQUENCES[id] * (variant() === "d" ? 2 : 1);
};
