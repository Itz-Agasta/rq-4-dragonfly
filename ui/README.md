# ui

Operator console. Vite + React + TypeScript, built to a static bundle that `dragonfly-core` serves on :8787 and Chromium runs fullscreen in kiosk mode.

Six screens: OPS, TWIN, ANALYSIS, SIMULATE, REPLAY, FLEET. **OPS is built**; the other five render a placeholder that says so. The theme tokens are `src/index.css`.

## Layout

```
src/
  components/ui/    shadcn primitives, installed by CLI and edited in place
  components/app/   shell, navigation rail, mission bar   (every screen)
  components/       anything more than one screen uses
  components/ops/   OPS only, including the mock values it still carries
  lib/              wire format, render loop, ring buffer, formatting, health poll
  store/            telemetry rings, cold app state, the channel registry
```

A component used by exactly one screen lives under that screen's folder.

## Telemetry never touches React state

Frames arrive at 20 Hz. Routing them through React would reconcile the tree twenty times a second, so instead they are written into a module store (`store/telemetry.ts`) and read inside a single shared `requestAnimationFrame` loop (`lib/live.tsx`). Readouts are DOM writes; charts are `uPlot.setData` calls. Neither re-renders.

Two consequences worth knowing before adding anything:

- **Non-finite values stop at the store.** Every optional field in the wire format is `NaN` when the controller does not measure it. A channel that goes non-finite holds its last real reading in the ring, so the chart stays drawable, and is flagged unavailable so the readout says nothing rather than `NaN`.
- **Anything drawing to a canvas must resolve theme tokens first.** Canvas `strokeStyle` ignores `var(--x)` silently and keeps the previous colour, so a trace styled with a CSS variable draws invisibly. SVG resolves variables; canvas does not.

## Freshness is not optional

A source that goes quiet leaves its last value on screen, so a frozen trace is indistinguishable from a steady one. Every channel knows which bus source owns it, and anything rendering a value consults that source's age. The link itself is polled from `/api/health` rather than inferred from socket silence: a core whose CAN interface never came up holds an open socket and sends nothing at all.

## Development

```bash
pnpm -F ui dev      # :5173, proxies /ws and /api to dragonfly-core on :8787
pnpm -F ui build
```

Needs `dragonfly-sim` and `dragonfly-core` running against a CAN interface. See the repository root.
