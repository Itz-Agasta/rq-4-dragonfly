import { useRef } from "react";

import type { FrameMap } from "../film/scroll";
import { at } from "../film/scroll";
import { DitherBg } from "./DitherBg";
import { PartLabels, TwinTrace } from "./EngineOverlay";
import { Scene } from "./Scene";

const BACK = 0.12; // dissolve from the injector macro back out to the open bay

const reveal: FrameMap = (p) =>
  p < BACK ? { a: at("S3", 1), b: at("S2", 1), mix: p / BACK } : { a: at("S2", 1) };

/** The product, named only now: the twin traces on over the real engine. */
export function Reveal() {
  return (
    <Scene vh={380} map={reveal} preload={["S2"]} stage="reveal" id="product">
      <TwinTrace />
      <div
        className="center"
        data-at="0.14,0.32"
        style={{ alignContent: "start", paddingTop: "15vh" }}
      >
        <span className="label">Introducing</span>
        <h1 className="wordmark-xl">DRAGONFLY</h1>
      </div>
      <PartLabels from={0.4} />
      <div className="legend" data-at="0.22,1.01">
        <span>
          <i className="solid" /> measured
        </span>
        <span>
          <i className="dashed" /> twin
        </span>
      </div>
      <div className="lower" data-at="0.55,1.01">
        <h2 className="h2" data-words>
          A twin that flies
          <br />
          beside the engine
        </h2>
        <p className="body">
          It predicts every reading from physics and watches <span className="accent">the gap</span>
          .
        </p>
      </div>
    </Scene>
  );
}

/** The dither interlude before the proof, armory's "built from the ground up" beat. */
export function FirstPrinciples() {
  const set = useRef<(r: number) => void>(() => {});
  // The cloud rises as the section dissolves in and holds while the line is read.
  const onP = (p: number) => set.current(Math.min(1, p * 3));
  return (
    <Scene vh={220} map={null} opaque stage="black" onP={onP}>
      <DitherBg bind={(s) => void (set.current = s)} />
      <div className="lower" data-at="0.12,1.01">
        <h2 className="h1 xl" data-words>
          Built from
          <br />
          <span className="accent">first principles</span>
        </h2>
        <p className="body">Every constant is tagged published or estimated.</p>
      </div>
    </Scene>
  );
}
