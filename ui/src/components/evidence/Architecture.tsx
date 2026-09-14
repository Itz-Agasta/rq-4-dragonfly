/**
 * The data path, drawn once.
 *
 * Hand-drawn SVG rather than a diagram library: it is nine boxes and four rules,
 * and a dependency that renders it would also bring its own type scale, its own
 * corner radii and its own idea of what an arrowhead looks like. Strokes and
 * type match the engine schematic on OPS, which is the same hand.
 *
 * The branch after RESIDUAL is the load-bearing part of the picture: detection,
 * health scoring and remaining life are three consumers of one quantity, not
 * three pipelines. Anything that reads a different quantity is wrong by
 * construction, and that is easier to see than to say.
 */

const BOX = { fill: "var(--card)", stroke: "var(--structure-hi)", strokeWidth: 1 };

function Box({
  x,
  y,
  w,
  h,
  title,
  sub,
}: {
  x: number;
  y: number;
  w: number;
  h: number;
  title: string;
  sub: string;
}) {
  return (
    <g>
      <rect x={x} y={y} width={w} height={h} {...BOX} />
      <text
        x={x + w / 2}
        y={y + h / 2 - 3}
        textAnchor="middle"
        fill="var(--foreground)"
        fontSize="10"
        letterSpacing="0.06em"
      >
        {title}
      </text>
      <text
        x={x + w / 2}
        y={y + h / 2 + 11}
        textAnchor="middle"
        fill="var(--foreground-dim)"
        fontSize="9"
      >
        {sub}
      </text>
    </g>
  );
}

/** A rule with a butt arrowhead. No curves, no marker defs for four arrows. */
function Arrow({ x1, y1, x2, y2 }: { x1: number; y1: number; x2: number; y2: number }) {
  return (
    <g stroke="var(--structure)" strokeWidth="1" fill="none">
      <path d={`M${x1} ${y1}H${x2}`} />
      <path d={`M${x2 - 4} ${y2 - 3}L${x2} ${y2}L${x2 - 4} ${y2 + 3}`} />
    </g>
  );
}

export function Architecture() {
  return (
    <figure className="border-border mx-auto max-w-[1100px] min-w-0 overflow-x-auto border">
      <svg
        viewBox="0 0 860 168"
        className="block w-full"
        style={{ fontFamily: "var(--font-mono)" }}
        role="img"
        aria-label="Data path from engine through the bus, the filter and the residual to the screens"
      >
        <rect width="860" height="168" fill="var(--background)" />

        <Box x={4} y={58} w={104} h={48} title="ENGINE" sub="dragonfly-sim" />
        <Arrow x1={108} y1={82} x2={128} y2={82} />

        <Box x={128} y={58} w={110} h={48} title="DroneCAN" sub="3 nodes · 20 Hz" />
        <Arrow x1={238} y1={82} x2={258} y2={82} />

        <Box x={258} y={58} w={104} h={48} title="INGEST" sub="dragonfly-core" />
        <Arrow x1={362} y1={82} x2={382} y2={82} />

        <Box x={382} y={58} w={104} h={48} title="UKF · 28" sub="twin-core" />
        <Arrow x1={486} y1={82} x2={506} y2={82} />

        <Box x={506} y={58} w={112} h={48} title="RESIDUAL" sub="vs healthy · 22 ch" />

        {/* The branch. One quantity, three consumers. */}
        <g stroke="var(--structure)" strokeWidth="1" fill="none">
          <path d="M618 82H638M638 26V138" />
        </g>
        <Arrow x1={638} y1={26} x2={666} y2={26} />
        <Arrow x1={638} y1={82} x2={666} y2={82} />
        <Arrow x1={638} y1={138} x2={666} y2={138} />

        <Box x={666} y={8} w={112} h={36} title="DETECT" sub="CUSUM · chi2" />
        <Box x={666} y={64} w={112} h={36} title="INDICES" sub="7 subsystems" />
        <Box x={666} y={120} w={112} h={36} title="RUL" sub="trend + interval" />

        <g stroke="var(--structure)" strokeWidth="1" fill="none">
          <path d="M778 26H800M778 82H800M778 138H800M800 26V138" />
        </g>
        <Arrow x1={800} y1={82} x2={824} y2={82} />

        <g>
          <rect x={824} y={20} width={32} height={124} {...BOX} />
          <text
            x={840}
            y={82}
            textAnchor="middle"
            fill="var(--foreground)"
            fontSize="10"
            letterSpacing="0.1em"
            transform="rotate(-90 840 82)"
          >
            SCREENS
          </text>
        </g>

        <text x={4} y={18} fill="var(--foreground-dim)" fontSize="9" letterSpacing="0.08em">
          DELETED IN A DEPLOYMENT
        </text>
        <path
          d="M4 24H238"
          stroke="var(--foreground-dim)"
          strokeWidth="1"
          strokeDasharray="3 3"
          fill="none"
        />
      </svg>
      <figcaption className="border-border label-micro border-t px-3 py-2">
        The simulator occupies the place a real engine and its FADEC occupy, and publishes to a real
        CAN interface. Replacing it with an engine deletes one crate and changes nothing downstream.
      </figcaption>
    </figure>
  );
}
