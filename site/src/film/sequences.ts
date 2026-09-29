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

const url = (id: SeqId, i: number) =>
  `${BASE}/${id}/${variant()}/${String(i + 1).padStart(4, "0")}.avif`;

type Seq = { frames: (HTMLImageElement | null)[]; started: boolean; loaded: number };
const store = new Map<SeqId, Seq>();
let decoded: (id: SeqId) => void = () => {};

/** The film canvas registers here to repaint when a better frame for its sequence lands. */
export const onDecoded = (cb: (id: SeqId) => void) => (decoded = cb);

function seq(id: SeqId): Seq {
  let s = store.get(id);
  if (!s) {
    s = { frames: Array.from({ length: SEQUENCES[id] }, () => null), started: false, loaded: 0 };
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

/** Starts loading a sequence once; resolves when its coarse pass (every 8th frame) is in. */
export function load(id: SeqId): Promise<void> {
  const s = seq(id);
  const n = SEQUENCES[id];
  if (s.started) return Promise.resolve();
  s.started = true;
  const idx = order(n);
  const coarse = Math.ceil(n / 8);
  let resolveCoarse!: () => void;
  const coarseDone = new Promise<void>((r) => (resolveCoarse = r));
  let next = 0;
  // Six in flight: enough to saturate a CDN connection, few enough that the coarse
  // pass of the scene the reader is on is not queued behind the fine pass.
  const pump = () => {
    if (next >= idx.length) return;
    const i = idx[next++];
    const img = new Image();
    img.decoding = "async";
    img.src = url(id, i);
    img
      .decode()
      .then(() => {
        s.frames[i] = img;
        decoded(id);
      })
      .catch(() => {})
      .finally(() => {
        if (++s.loaded === coarse) resolveCoarse();
        pump();
      });
  };
  for (let k = 0; k < 6; k++) pump();
  return coarseDone;
}

/** Nearest decoded frame to `i`, searching outward, or null if none has landed yet. */
export function nearest(id: SeqId, i: number): HTMLImageElement | null {
  const f = seq(id).frames;
  for (let d = 0; d < f.length; d++) {
    const a = f[i - d];
    if (a) return a;
    const b = f[i + d];
    if (b) return b;
  }
  return null;
}

/** Fraction of a sequence decoded, 0..1. Drives the preloader, nothing else. */
export const progress = (id: SeqId) => seq(id).loaded / SEQUENCES[id];
