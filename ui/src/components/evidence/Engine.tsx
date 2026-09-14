/**
 * 01 ENGINE. What is modelled, what is certificated about it, and the one point
 * where both numbers exist.
 *
 * The rating point is placed first and the deltas are stated rather than buried,
 * because a comparison that shows a delta is evidence someone measured and a
 * comparison that shows none is evidence of nothing.
 */

import { Plot, useCharts } from "@/components/evidence/Chart";
import { IDENTITY, PORTABILITY, RATING, REDLINES } from "@/components/evidence/data/engine";
import {
  Callout,
  Cell,
  Defs,
  Link,
  P,
  Quote,
  Row,
  Section,
  Stats,
  Table,
} from "@/components/evidence/parts";

function Comparisons({ rows, caption }: { rows: typeof RATING; caption: string }) {
  return (
    <div className="flex min-w-0 flex-col gap-2">
      <span className="label-micro">{caption}</span>
      <Table
        columns={["Quantity", "Published", "Modelled", "Delta", "Note"]}
        align={["l", "r", "r", "r", "l"]}
      >
        {rows.map((r) => (
          <Row key={r.quantity}>
            <Cell>{r.quantity}</Cell>
            <Cell numeric>{r.published}</Cell>
            <Cell numeric>{r.modelled}</Cell>
            {/* Deliberately not accented: `+1.7%` in orange reads as a fault
                rather than as a result somebody measured. Luminance instead. */}
            <Cell numeric>{r.delta || "·"}</Cell>
            <Cell dim wide>
              {r.note}
            </Cell>
          </Row>
        ))}
      </Table>
    </div>
  );
}

export function Engine() {
  const charts = useCharts();

  return (
    <Section
      id="engine"
      index="01"
      title="The modelled engine and its type certificate"
      standfirst="The Austro Engine E4P holds EASA type certificate E.200. Every published figure in this section comes from that certificate or from the manufacturer factsheet, and both are in the public record."
    >
      <Stats
        items={[
          ["+1.7%", "modelled against certificated at the rating point"],
          ["11,000 ft", "rated power held to the rated critical altitude"],
          ["5.5M", "flight hours on the E4 series"],
        ]}
      />

      <div className="lattice" style={{ gridTemplateColumns: "repeat(3, minmax(0, 1fr))" }}>
        <section className="cell flex flex-col gap-1.5">
          <span className="label-micro">Modelled engine</span>
          <span className="text-foreground text-[15px]">{IDENTITY.name}</span>
        </section>
        <section className="cell flex flex-col gap-1.5">
          <span className="label-micro">Type certificate</span>
          <span className="text-[15px]">
            <Link href={IDENTITY.certificateHref}>{IDENTITY.certificate}</Link>
          </span>
        </section>
        <section className="cell flex flex-col gap-1.5">
          <span className="label-micro">Block</span>
          <span className="text-foreground text-[15px]">{IDENTITY.block}</span>
        </section>
      </div>

      <Quote>
        Every published figure below was written down by someone else first. The source document is
        public.
      </Quote>

      <P>{IDENTITY.summary}</P>
      <P>{IDENTITY.airframe}</P>

      <Comparisons rows={RATING} caption="Rating point and envelope" />

      <Callout label="On the deltas, and on the word hp">
        The nameplate reads 180 hp, and that is metric PS. In mechanical horsepower 132 kW is 177.0
        hp, not 180. Quoting whichever unit makes the agreement look exact would be an unearned win,
        so power is carried in kW throughout and the rating point is reported as{" "}
        <span className="num text-foreground">
          134.2 kW modelled against 132.0 kW certificated, +1.7%
        </span>
        . The model runs at full fuelling command and is not trimmed to the certificate.
      </Callout>

      <Plot
        id="power_altitude"
        charts={charts}
        caption="Brake power against altitude at full fuelling, 3880 rpm. The dashed lines are the 180 hp rating and the rated critical altitude, not model output. Hover to read the power at any altitude."
      />

      <Comparisons rows={REDLINES} caption="Certificated limits, E4P" />

      <Callout label="Why two of these are estimated">
        The certificate limits oil, coolant, fuel and speed. Neither it nor the Operation Manual
        publishes an exhaust or head temperature limit, because on this engine the EECU fuelling
        limiter holds both and no pilot ever sees them. Both are therefore estimated, and marked
        estimated everywhere they appear: in the parameter file, on this screen, and on the alarm
        that uses them.
      </Callout>

      <div className="lattice mt-2" style={{ gridTemplateColumns: "repeat(2, minmax(0, 1fr))" }}>
        <section className="cell flex flex-col gap-3">
          <span className="label-micro">Why this engine was modelled</span>
          <Defs items={PORTABILITY.built} />
        </section>
        <section className="cell flex flex-col gap-3">
          <span className="label-micro">Pointing it at the indigenous engine</span>
          <Defs items={PORTABILITY.target} />
        </section>
      </div>
    </Section>
  );
}
