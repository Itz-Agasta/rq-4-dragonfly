/**
 * Engine line work: inline-four, heavy-fuel compression ignition, turbocharged,
 * side elevation.
 *
 * Real geometry rather than labelled rectangles. Four bores with pistons at
 * different heights and ring lines, angled connecting rods onto a crank with
 * visible throws, journals and counterweights, four intake runners off a plenum,
 * an exhaust log into the turbine snout, a volute turbocharger whose compressor
 * and turbine sit on one visible shaft and centreline, an intercooler with core
 * fins, and a sump with a pickup. The drawing is a claim about the model behind
 * it; a placeholder here would say the engineering is a placeholder too.
 *
 * Drawn on a 1120x610 canvas and scaled to fit, so every coordinate below is in
 * that space and independent of the panel's size.
 */

/** Horizontal pitch between bore centres, drawing units. */
const PITCH = 100;

/** Left edge of cylinder 1's bore. */
const BORE_X0 = 298;

/** Bore width. */
const BORE_W = 64;

/**
 * Crown height of each piston, drawing units.
 *
 * Deliberately all different: four pistons drawn at the same height is the single
 * fastest way to tell an engine person the drawing is decorative. These are the
 * positions a 1-3-4-2 firing order puts them in at one instant.
 */
const PISTON_Y = [264, 309, 279, 316];

/** Big-end journal positions, matching the rods below. */
const JOURNAL = [
  { x: 330, y: 399 },
  { x: 448, y: 443 },
  { x: 552, y: 412 },
  { x: 630, y: 451 },
];

const STRUCTURE = "var(--structure)";
const KEY_EDGE = "var(--structure-hi)";
const FAINT = "var(--border)";
const PANEL = "var(--card)";

function boreLeft(i: number): number {
  return BORE_X0 + i * PITCH;
}

function Cylinder({ index, accent }: { index: number; accent: boolean }) {
  const left = boreLeft(index);
  const right = left + BORE_W;
  const centre = left + BORE_W / 2;
  const top = PISTON_Y[index]!;
  const stroke = accent ? "var(--primary)" : KEY_EDGE;
  const ringStroke = accent ? "var(--primary)" : STRUCTURE;

  return (
    <g>
      {/* bore walls */}
      <line
        x1={left}
        y1="240"
        x2={left}
        y2="380"
        stroke={stroke}
        strokeWidth={accent ? 1.75 : 1.25}
      />
      <line
        x1={right}
        y1="240"
        x2={right}
        y2="380"
        stroke={stroke}
        strokeWidth={accent ? 1.75 : 1.25}
      />
      {/* water jacket */}
      <line x1={left - 8} y1="246" x2={left - 8} y2="374" stroke={STRUCTURE} />
      <line x1={right + 8} y1="246" x2={right + 8} y2="374" stroke={STRUCTURE} />
      {/* piston with ring lines and gudgeon pin */}
      <rect
        x={left + 1}
        y={top}
        width="62"
        height="34"
        fill={PANEL}
        stroke={stroke}
        strokeWidth={accent ? 1.5 : 1.25}
      />
      <line
        x1={left + 1}
        y1={top + 8}
        x2={right - 1}
        y2={top + 8}
        stroke={ringStroke}
        strokeWidth="1"
        opacity={accent ? 0.6 : 1}
      />
      <line
        x1={left + 1}
        y1={top + 13}
        x2={right - 1}
        y2={top + 13}
        stroke={ringStroke}
        strokeWidth="1"
        opacity={accent ? 0.6 : 1}
      />
      <line
        x1={left + 1}
        y1={top + 18}
        x2={right - 1}
        y2={top + 18}
        stroke={ringStroke}
        strokeWidth="1"
        opacity={accent ? 0.6 : 1}
      />
      {!accent && <circle cx={centre} cy={top + 17} r="5" stroke={KEY_EDGE} />}
      {/* connecting rod down to its journal */}
      <path
        d={`M${centre} ${top + 17} L${JOURNAL[index]!.x} ${JOURNAL[index]!.y}`}
        stroke={accent ? "var(--primary)" : STRUCTURE}
        strokeWidth="2.5"
      />
      {/* corner marks calling out the one cylinder that matters */}
      {accent ? (
        <path
          d={`M${left - 18} 258 L${left - 18} 244 L${left - 4} 244 M${right + 18} 244 L${right + 4} 244 M${right + 18} 244 L${right + 18} 258
              M${left - 18} 362 L${left - 18} 376 L${left - 4} 376 M${right + 18} 376 L${right + 4} 376 M${right + 18} 376 L${right + 18} 362`}
          stroke="var(--primary)"
          strokeWidth="1.25"
        />
      ) : null}
    </g>
  );
}

export function SchematicDrawing({
  faultCylinder,
  showDotGrid,
}: {
  /** 1-based cylinder to accent, or 0 for none. */
  faultCylinder: number;
  showDotGrid: boolean;
}) {
  return (
    <svg
      viewBox="0 0 1120 610"
      preserveAspectRatio="xMidYMid meet"
      className="block min-h-0 flex-1"
      width="100%"
      height="100%"
      role="img"
      aria-label="Engine schematic, inline four cylinder turbocharged compression ignition"
    >
      <defs>
        <pattern id="df-dots" width="22" height="22" patternUnits="userSpaceOnUse">
          <circle cx="0.7" cy="0.7" r="0.7" fill="var(--grid)" />
        </pattern>
        <pattern id="df-fins" width="11" height="8" patternUnits="userSpaceOnUse">
          <line x1="0.5" y1="0" x2="0.5" y2="8" stroke={STRUCTURE} strokeWidth="1" />
        </pattern>
      </defs>

      {showDotGrid ? <rect x="0" y="0" width="1120" height="610" fill="url(#df-dots)" /> : null}

      <g fill="none" stroke={STRUCTURE} strokeWidth="1.25" strokeLinejoin="miter">
        {/* charge-air pipe: intercooler out to the plenum */}
        <path d="M748 400 L748 578 L52 578 L52 129 L120 129" stroke={KEY_EDGE} />
        <path d="M760 400 L760 562 L68 562 L68 145 L120 145" stroke={KEY_EDGE} />

        {/* intake plenum */}
        <rect x="120" y="118" width="570" height="38" fill={PANEL} stroke={KEY_EDGE} />
        <line x1="120" y1="127" x2="690" y2="127" stroke={FAINT} />

        {/* two intake runners per cylinder */}
        {[0, 1, 2, 3].map((i) => {
          const a = 286 + i * PITCH;
          return (
            <g key={`runner-${i}`}>
              <path d={`M${a} 156 C${a} 176 ${a + 17} 172 ${a + 17} 190`} />
              <path d={`M${a + 14} 156 C${a + 14} 172 ${a + 31} 176 ${a + 31} 190`} />
            </g>
          );
        })}

        {/* exhaust log and the snout into the turbine volute */}
        <rect x="330" y="62" width="390" height="42" fill={PANEL} stroke={KEY_EDGE} />
        <line x1="330" y1="71" x2="720" y2="71" stroke={FAINT} />
        <path d="M720 66 L766 88" stroke={KEY_EDGE} />
        <path d="M720 104 L759 121" stroke={KEY_EDGE} />

        {/* exhaust risers, drawn over the plenum */}
        {[0, 1, 2, 3].map((i) => {
          const x = 343 + i * PITCH;
          return (
            <g key={`riser-${i}`}>
              <rect x={x} y="104" width="18" height="86" fill={PANEL} />
              <line x1={x} y1="104" x2={x} y2="190" />
              <line x1={x + 18} y1="104" x2={x + 18} y2="190" />
            </g>
          );
        })}

        {/* head, with two valves and a central injector per cylinder */}
        <rect x="275" y="190" width="410" height="50" fill={PANEL} stroke={KEY_EDGE} />
        {[0, 1, 2, 3].map((i) => {
          const c = 330 + i * PITCH;
          return (
            <g key={`valves-${i}`}>
              <path d={`M${c - 24} 238 L${c - 12} 196 M${c - 31} 238 L${c - 17} 238`} />
              <path d={`M${c + 24} 238 L${c + 12} 196 M${c + 17} 238 L${c + 31} 238`} />
              <path d={`M${c} 196 L${c} 234`} stroke={KEY_EDGE} />
            </g>
          );
        })}

        {/* block outer walls */}
        <line x1="275" y1="240" x2="275" y2="380" stroke={KEY_EDGE} />
        <line x1="685" y1="240" x2="685" y2="380" stroke={KEY_EDGE} />

        {/* crankcase */}
        <path d="M265 380 L275 380 M685 380 L695 380" stroke={KEY_EDGE} />
        <path d="M265 380 L265 470 L695 470 L695 380" stroke={KEY_EDGE} />

        {/* crankshaft: centreline, main bearings, throws, counterweights */}
        <line x1="272" y1="425" x2="688" y2="425" stroke={FAINT} strokeDasharray="8 5" />
        {[280, 380, 480, 580, 680].map((x) => (
          <circle key={`main-${x}`} cx={x} cy="425" r="11" stroke={KEY_EDGE} />
        ))}
        {[330, 430, 530, 630].map((x) => (
          <circle key={`throw-${x}`} cx={x} cy="425" r="26" stroke={FAINT} />
        ))}
        <path d="M358.6 441.5 A33 33 0 0 1 301.4 441.5" strokeWidth="7" />
        <path d="M398.1 433.5 A33 33 0 0 1 438.5 393.1" strokeWidth="7" />
        <path d="M530 458 A33 33 0 0 1 501.4 408.5" strokeWidth="7" />
        <path d="M601.4 408.5 A33 33 0 0 1 658.6 408.5" strokeWidth="7" />
        {JOURNAL.map((j) => (
          <circle key={`journal-${j.x}`} cx={j.x} cy={j.y} r="9" fill={PANEL} stroke={KEY_EDGE} />
        ))}

        {/* sump, pickup and oil level */}
        <path d="M300 470 L660 470 L620 530 L340 530 Z" stroke={KEY_EDGE} />
        <path d="M480 460 L480 512" />
        <rect x="462" y="512" width="36" height="9" stroke={KEY_EDGE} />
        <line x1="330" y1="505" x2="630" y2="505" stroke={FAINT} strokeDasharray="6 5" />

        {/* turbocharger: one shaft, one centreline, through both wheels */}
        <line x1="821" y1="34" x2="821" y2="316" stroke={FAINT} strokeDasharray="8 5" />
        <rect x="800" y="150" width="42" height="50" fill={PANEL} stroke={KEY_EDGE} />
        <line x1="821" y1="146" x2="821" y2="204" stroke={KEY_EDGE} strokeWidth="2.5" />
        <line x1="806" y1="162" x2="836" y2="162" stroke={FAINT} />
        <line x1="806" y1="188" x2="836" y2="188" stroke={FAINT} />

        {/* turbine volute, wheel and downpipe */}
        <path d="M757 122 A68 68 0 1 1 875 118" stroke={KEY_EDGE} />
        <path d="M769 118 A54 54 0 1 1 863 114" />
        <circle cx="821" cy="100" r="30" stroke={KEY_EDGE} />
        <path d="M821 71 L821 82 M850 100 L839 100 M821 129 L821 118 M792 100 L803 100 M842 79 L834 87 M842 121 L834 113 M800 121 L808 113 M800 79 L808 87" />
        <circle cx="821" cy="100" r="7" fill={STRUCTURE} stroke="none" />
        <path d="M871 130 L897 156 M851 146 L877 172 M877 172 L897 156" stroke={KEY_EDGE} />

        {/* compressor volute, wheel, inlet and outlet duct */}
        <path d="M763 232 A60 60 0 1 0 875 236" stroke={KEY_EDGE} />
        <path d="M775 238 A48 48 0 1 0 863 240" />
        <circle cx="821" cy="250" r="26" stroke={KEY_EDGE} />
        <path d="M821 225 L821 235 M846 250 L836 250 M821 275 L821 265 M796 250 L806 250 M839 232 L832 239 M839 268 L832 261 M803 268 L810 261 M803 232 L810 239" />
        <circle cx="821" cy="250" r="6" fill={STRUCTURE} stroke="none" />
        <path d="M849 236 L908 236 M849 264 L908 264 M908 230 L908 270" stroke={KEY_EDGE} />
        <path d="M843 300 L843 330 M871 300 L871 330" stroke={KEY_EDGE} />

        {/* intercooler with core fins */}
        <rect x="730" y="330" width="280" height="70" fill={PANEL} stroke={KEY_EDGE} />
        <rect x="748" y="330" width="244" height="70" fill="url(#df-fins)" stroke="none" />
        <line x1="748" y1="330" x2="748" y2="400" stroke={KEY_EDGE} />
        <line x1="992" y1="330" x2="992" y2="400" stroke={KEY_EDGE} />
        <line x1="748" y1="344" x2="992" y2="344" stroke={FAINT} />
        <line x1="748" y1="386" x2="992" y2="386" stroke={FAINT} />
      </g>

      <g fill="none">
        {[0, 1, 2, 3].map((i) => (
          <Cylinder key={`cyl-${i}`} index={i} accent={faultCylinder === i + 1} />
        ))}
      </g>

      {/* drawing corner marks */}
      <g stroke={KEY_EDGE} strokeWidth="1" fill="none">
        <path d="M26 26 L26 40 M19 33 L33 33" />
        <path d="M1094 26 L1094 40 M1087 33 L1101 33" />
        <path d="M26 570 L26 584 M19 577 L33 577" />
        <path d="M1094 570 L1094 584 M1087 577 L1101 577" />
      </g>
    </svg>
  );
}
