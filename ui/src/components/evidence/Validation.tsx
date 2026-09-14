/**
 * 03 VALIDATION. The physics gate, and what it does not establish.
 *
 * Every figure plots `charts.json`, which `just validate` writes in the same call
 * that writes the SVG beside it. The interactive chart and the figure embedded in
 * `docs/model_validation.md` therefore cannot disagree. Transcribing the series
 * into this file by hand would make the screen a second source of truth for
 * numbers only the model is entitled to produce.
 */

import { Plot, useCharts } from "@/components/evidence/Chart";
import { Equation } from "@/components/evidence/Equation";
import { Callout, Cell, P, Quote, Row, Section, Stats, Table } from "@/components/evidence/parts";

/**
 * Reference operating points, from `docs/model_validation.md` section 7.
 *
 * The hot-day row is the binding thermal case and is kept in because it is the
 * one that reads wrong to intuition: cooling binds low, hot and slow, not high.
 */
const POINTS: [string, string, string, string, string, string, string, string][] = [
  ["sea level, take-off", "180.0", "557", "3.100", "140,082", "1.562", "39.3", "986"],
  ["11,000 ft, full", "181.2", "560", "3.101", "169,722", "1.602", "39.3", "971"],
  ["22,400 ft, cruise", "63.2", "204", "1.360", "125,521", "2.013", "15.1", "760"],
  ["sea level, ISA+30 climb", "171.1", "529", "3.100", "151,346", "1.414", "39.3", "1070"],
];

const COLUMNS = [
  "Point",
  "Power hp",
  "Prop N.m",
  "MAP bar",
  "Turbo rpm",
  "Lambda",
  "Fuel L/h",
  "EGT K",
];

export function Validation() {
  const charts = useCharts();

  return (
    <Section
      id="validation"
      index="03"
      title="What the model reproduces, and what it does not"
      standfirst="Every figure and number in this section comes from sweeping the model across the certified envelope. Changing one of them means changing the model."
    >
      <Stats
        items={[
          ["180.0 hp", "held flat from sea level to 11,000 ft"],
          ["1.65 s", "turbocharger spool to 90% of the boost rise"],
          ["96.5 hp", "still making power at 32,000 ft"],
        ]}
      />

      <Quote>
        The knee at 11,000 ft falls out of the hardware. Compressor sizing meets the shaft speed
        limit at that altitude, and moving it means resizing the compressor.
      </Quote>

      <P>
        A power curve with a bend in it proves nothing on its own. The mechanism does. Below the
        critical altitude the wastegate modulates to hold the manifold set-point, and shaft speed
        climbs as the compressor is asked for more pressure ratio against thinner air. At 11,000 ft
        the shaft reaches the speed the controller supervises it to, and the wastegate reopens to
        protect it.
      </P>

      <Plot
        id="turbo_altitude"
        charts={charts}
        caption="The plateau ends where the shaft reaches its supervised speed, not at a parameter. Hover to read shaft speed at any altitude."
      />

      <Plot
        id="temperatures_altitude"
        charts={charts}
        caption="Exhaust temperature rises above the critical altitude and head temperature falls. As boost falls the excess air ratio falls with it, so a smoke-limited engine runs hotter at altitude, while the head tracks fuel energy and runs cooler."
      />

      <Plot
        id="spool"
        charts={charts}
        caption="A step from 30% to full fuelling at 3000 rpm reaches 90% of the boost rise in 1.65 s. Crank speed is governed, so the trace follows the shaft and the manifold, not the engine accelerating."
      />

      <div className="flex min-w-0 flex-col gap-2">
        <span className="label-micro">Reference operating points</span>
        <Table columns={COLUMNS} align={["l", "r", "r", "r", "r", "r", "r", "r"]}>
          {POINTS.map((p) => (
            <Row key={p[0]}>
              <Cell>{p[0]}</Cell>
              {COLUMNS.slice(1).map((label, i) => (
                <Cell key={label} numeric>
                  {p[i + 1]}
                </Cell>
              ))}
            </Row>
          ))}
        </Table>
      </div>

      <Equation
        tex={String.raw`\dot{m}_{\mathrm{rad}} \;=\; m\,\rho\,V \qquad\text{and}\qquad V_{\mathrm{true}} \;=\; V_{\mathrm{ind}}\big/\sqrt{\rho/\rho_0}`}
        note="Why altitude does not bind the cooling. True airspeed rises as density falls, so the product scales with the square root of density rather than with density itself."
      />

      <Callout label="The hot day is the binding case, and the intuition points the other way">
        Cooling is liquid, so radiator air mass flow goes as m·rho·V. At constant indicated
        airspeed, true airspeed rises as density falls, so that product scales with the square root
        of density rather than with density. At 25,000 ft it is about two thirds of the sea-level
        value against a temperature difference nearly two thirds larger, while the heat to be
        rejected has already fallen with the engine's own power. High altitude is therefore not
        where cooling binds. Hot, low, slow and at full power is.
      </Callout>

      <Plot
        id="bsfc"
        charts={charts}
        caption="One curve per engine speed. A filled contour carries the same information and makes it harder to follow a single speed across the load range."
      />

      <Equation
        tex={String.raw`\eta_{\mathrm{vol}} \;=\; c_1\sqrt{p_{\mathrm{im}}} \;+\; c_2\sqrt{\omega_e} \;+\; c_3`}
        note="Volumetric efficiency, Wahlstrom & Eriksson 2011 eq. 9. The coefficients are [6.0e-5, -0.0125, 1.1400], fitted over the cruise to take-off band."
      />

      <Plot
        id="volumetric_efficiency"
        charts={charts}
        caption="Clamped below about 1500 rpm, where the parametric form extrapolates through unity. Above that band it is fitted across cruise to take-off."
      />

      <Callout label="What this establishes, and what it does not">
        <p>
          Established: the model reproduces the published rating point on power, propeller torque
          and fuel consumption simultaneously, from a fit to that point alone. It holds rated power
          to the rated critical altitude through a mechanism the turbocharger figure shows directly.
          It stays physical across the whole certified envelope.
        </p>
        <p className="mt-2">
          Not established: anything at part load or at altitude. No measurement exists to check
          against, because none is published for this engine. The parameters marked estimated come
          from engine class, fitted to the single published point. They are plausible, not measured,
          and the parameter file says so for each one.
        </p>
      </Callout>

      <P>
        The sweep runs to 32,000 ft because that is the Medium Altitude Long Endurance mission
        envelope, while the type certificate limits this engine to 20,000 ft. Everything above that
        line is extrapolation, marked as extrapolation in the generated document, and must not be
        quoted as certified performance.
      </P>
    </Section>
  );
}
