/**
 * 04 PROVENANCE. Where every number comes from, what checks it, and the answer
 * to the dataset question.
 *
 * The dataset answer is placed before the coverage map on purpose. It is the
 * first thing a reviewer asks and the thing most often fudged, so it is stated
 * plainly and early rather than left to be discovered after the good news.
 */

import { Architecture } from "@/components/evidence/Architecture";
import {
  DATA_ANSWER,
  PIPELINE,
  PS_COVERAGE,
  VERIFICATION,
} from "@/components/evidence/data/provenance";
import { Equation } from "@/components/evidence/Equation";
import {
  Callout,
  Cell,
  Defs,
  P,
  Quote,
  Row,
  Section,
  Stats,
  Table,
} from "@/components/evidence/parts";

/** Test totals, from `cargo test --workspace`. Restate after a run, never guess. */
const TESTS = { passed: 254, ignored: 3, suites: 20 };

function Pipeline() {
  return (
    <div className="flex min-w-0 flex-col gap-2">
      <span className="label-micro">Stage by stage</span>
      <Table columns={["Stage", "Owner", "Rate", "What it does"]} align={["l", "l", "r", "l"]}>
        {PIPELINE.map((s) => (
          <Row key={s.id}>
            <Cell>{s.title}</Cell>
            <Cell dim>{s.owner}</Cell>
            <Cell numeric dim>
              {s.rate}
            </Cell>
            <Cell dim wide>
              {s.detail}
            </Cell>
          </Row>
        ))}
      </Table>
    </div>
  );
}

function Coverage() {
  return (
    <div className="lattice" style={{ gridTemplateColumns: "repeat(2, minmax(0, 1fr))" }}>
      {PS_COVERAGE.map((c) => (
        <section key={c.section} className="cell flex flex-col gap-3">
          <div className="flex items-baseline gap-2">
            <span className="label-micro num text-structure-hi">{c.section}</span>
            <span className="label-micro text-foreground">{c.title}</span>
          </div>
          <dl className="flex flex-col gap-3">
            {c.items.map(([bullet, answer]) => (
              <div key={bullet}>
                <dt className="text-foreground text-[13px]">{bullet}</dt>
                <dd className="text-muted-foreground mt-0.5 text-[13px] leading-[1.6]">{answer}</dd>
              </div>
            ))}
          </dl>
        </section>
      ))}
    </div>
  );
}

export function Provenance() {
  return (
    <Section
      id="provenance"
      index="04"
      title="Where every number comes from"
      standfirst="Every readout in this application is one of four things: a value off the bus, a value the physics model computed, a value the estimator inferred, or a value that was authored. Exactly one readout across all seven screens is the fourth kind, and it is labelled on the screen it appears on."
    >
      <Architecture />
      <Pipeline />

      <Callout label="Why the residual and not the innovation">
        An estimator that also tracks health parameters drives its own innovation to zero whether or
        not the machine is degraded. A display fed that innovation shows a coked injector as nothing
        at all. The monitor falls silent at the moment it is most needed. Every display and every
        detector here reads the residual against a nominal healthy engine instead, and the
        innovation serves one purpose, which is deciding whether the filter has locked.
      </Callout>

      <div className="flex flex-col gap-3">
        <span className="label-micro">{DATA_ANSWER.question}</span>
        <Quote>{DATA_ANSWER.headline}</Quote>
      </div>

      {DATA_ANSWER.body.map((para) => (
        <P key={para.slice(0, 40)}>{para}</P>
      ))}

      <div className="lattice" style={{ gridTemplateColumns: "repeat(2, minmax(0, 1fr))" }}>
        <section className="cell flex flex-col gap-3">
          <span className="label-micro">Data this system does produce</span>
          <Defs items={DATA_ANSWER.ownData} />
        </section>
        <section className="cell flex flex-col gap-3">
          <span className="label-micro">Not built, and stated here</span>
          <Defs items={DATA_ANSWER.roadmap} />
        </section>
      </div>

      <div className="flex min-w-0 flex-col gap-2">
        <span className="label-micro">What is checked, and by what</span>
        <Table columns={["Claim", "How it is held"]} align={["l", "l"]}>
          {VERIFICATION.map((v) => (
            <Row key={v.claim}>
              <Cell wide>{v.claim}</Cell>
              <Cell dim wide>
                {v.how}
              </Cell>
            </Row>
          ))}
        </Table>
      </div>

      <Stats
        items={[
          [`${TESTS.passed}`, `tests passing across ${TESTS.suites} suites`],
          ["1.30 ms", "twin cost per frame, 2.6% of a core at 20 Hz"],
          ["48 kB/s", "ingest per airframe, measured on the wire"],
        ]}
      />

      <Equation
        tex={String.raw`S_k^{+} \;=\; \max\!\left(0,\; S_{k-1}^{+} + z_k - \kappa\right), \qquad \kappa = \tfrac{1}{2}\delta`}
        note="The cumulative sum test, run on each of the 22 channels. z is the residual in that channel's own standard deviations, and kappa is slack at half the shift to be detected. That is Page's classical choice and gives an in-control run length near 465 samples per channel."
      />

      <Equation
        tex={String.raw`d_k^{2} \;=\; r_k^{\mathsf{T}} \Sigma^{-1} r_k \;\sim\; \chi^{2}_{22}`}
        note="The whole-vector test, run alongside the per-channel one. A fault spread thinly across many channels moves none of them far enough to trip a CUSUM but still moves this."
      />

      <Callout label="A green suite is not evidence that a detector works">
        Unit tests feed zero-mean noise and a real bus does not. Two detector defects in this
        project were invisible to a passing suite and obvious within seconds of a live run. Every
        detection claim therefore names the profile it was measured on, and is measured at real
        time. On a compressed time base the covariance inflation pins at its ceiling, every sigma
        grows ten times larger, and a broken detector looks calm.
      </Callout>

      <div className="flex flex-col gap-3">
        <span className="label-micro">Problem statement coverage, section by section</span>
        <Coverage />
      </div>
    </Section>
  );
}
