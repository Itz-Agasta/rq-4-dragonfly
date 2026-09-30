import type { FrameMap } from "../film/scroll";
import { at, linear } from "../film/scroll";
import { ChannelBoxes, ENGINE_FX } from "./EngineOverlay";
import { Scene } from "./Scene";

// Act I, the problem. One continuous camera: the plateau, the aircraft, into the
// engine bay, and a dive into cylinder 3 that dissolves into the injector macro.

/** Cold open: descent through the haze, the aircraft settles into frame. */
export function Hero() {
  return (
    <Scene vh={460} map={linear("S1")} preload={["S1"]} id="top">
      <div className="lower" data-at="-1,0.24">
        <h1 className="h1">
          Engine faults start
          <br />
          before any gauge moves
        </h1>
        <p className="body">A long-endurance drone flies thirty hours on one piston engine.</p>
      </div>
      <div className="lower" data-at="0.5,1.01">
        <h2 className="h2" data-words>
          Redlines are its
          <br />
          only warning
        </h2>
        <p className="body">They fire once a reading crosses its limit.</p>
      </div>
    </Scene>
  );
}

// Engine scene timeline, as fractions of its scroll.
const IN = 0.04; // dissolve from S1's last frame into S2
// S2 is already moving through the dissolve, so the camera never stops at the
// seam. Kept short: the frames differ in grade and a few pixels of pose (SSIM
// 0.91), and a long dissolve shows both aircraft at once.
const LEAD = 0.02;
const DIVE = 0.8; // start of the push into cylinder 3
const CROSS = 0.88; // start of the dissolve into the injector macro
// Cylinder 3's bore, measured in footage space (EngineOverlay BORES[2]).
const CYL3 = { x: 605, y: 520 };

const engine: FrameMap = (p) => {
  if (p < IN) return { a: at("S1", 1), b: at("S2", (p / IN) * LEAD), mix: p / IN, fxB: ENGINE_FX };
  const u = LEAD + ((p - IN) / (DIVE - IN)) * (1 - LEAD);
  if (p < DIVE) return { a: at("S2", u), fx: ENGINE_FX };
  const d = (p - DIVE) / (1 - DIVE);
  // Ease-in on the push, so it starts as a drift and ends as a dive.
  const zoom = { ...CYL3, s: 1 + 2.2 * d * d };
  if (p < CROSS) return { a: at("S2", 1), zoom, fx: ENGINE_FX };
  return { a: at("S2", 1), zoom, fx: ENGINE_FX, b: at("S3", 0), mix: (p - CROSS) / (1 - CROSS) };
};

/** Orbit into the engine bay; every instrumented part is bracketed, then the camera dives. */
export function Engine() {
  return (
    <Scene vh={440} map={engine} preload={["S2", "S3"]}>
      <div className="lower" data-at="0.1,0.4">
        <h2 className="h2" data-words>
          A certified
          <br />
          aero diesel
        </h2>
        <p className="body spec">Austro E4P · 1,991 cm³ · 132 kW · 3,880 rpm</p>
      </div>
      <ChannelBoxes from={0.46} to={0.79} />
      <div className="lower" data-at="0.5,0.79">
        <h2 className="h2" data-words>
          22 channels,
          <br />
          all in limits
        </h2>
      </div>
    </Scene>
  );
}

/** Inside cylinder 3: the nozzle cokes, the spray thins, and nothing trips. */
export function Fault() {
  return (
    <Scene vh={340} map={linear("S3")} preload={["S3", "S2"]}>
      <div className="lower" data-at="0.04,0.4">
        <div>
          <span className="label accent">Cylinder 3</span>
          <h2 className="h2" data-words>
            The injector is coking
          </h2>
        </div>
      </div>
      <div className="lower" data-at="0.46,0.74">
        <h2 className="h2" data-words>
          It runs cooler,
          <br />
          not hotter
        </h2>
        <p className="body">Less fuel. This engine's limits only catch readings that rise.</p>
      </div>
      <div className="center" data-at="0.78,1.01">
        <h2 className="h1" data-words>
          <span className="accent">Nothing will ever trip</span>
        </h2>
      </div>
    </Scene>
  );
}
