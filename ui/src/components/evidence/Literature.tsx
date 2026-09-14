/**
 * 05 LITERATURE. Every source, with the file that cites it.
 *
 * The `cited` column is what makes this a claim rather than a reading list. A
 * reviewer who doubts a row opens that file and finds the same reference sitting
 * over the equation it justifies.
 */

import { LITERATURE, PAPER_COUNT } from "@/components/evidence/data/citations";
import { P, Quote, Section, Stats } from "@/components/evidence/parts";

export function Literature() {
  return (
    <Section
      id="literature"
      index="05"
      title="The literature each part rests on"
      standfirst={`${PAPER_COUNT} sources. Each reference is kept in the source file that uses it, on the line above the equation or constant it supports. Every link resolves to a DOI or a tagged specification.`}
    >
      <Stats
        items={[
          [`${PAPER_COUNT}`, "sources, each pinned to a DOI or a specification"],
          [`${LITERATURE.length}`, "subsystems, every one answerable to a paper"],
          ["38", "citations in the Rust sources, each above its own equation"],
        ]}
      />

      <Quote>
        A fault injected ten times faster than the real one still produces a convincing detection.
        Every fault in this system is rated against a published measurement of the real thing.
      </Quote>

      <P>
        Injector coking is the clearest case. Functional failure takes hundreds of hours on
        unadditised service fuel, and the accelerated mission that demonstrates it compresses the
        depth without changing the shape. That distinction is recorded in the test that runs it, so
        the claim stays a claim about the estimator.
      </P>

      {LITERATURE.map((group) => (
        <div key={group.title} className="flex min-w-0 flex-col gap-2">
          <div className="border-border flex flex-wrap items-baseline gap-x-3 gap-y-1 border-b pb-1.5">
            <span className="label-micro text-foreground">{group.title}</span>
            <span className="label-micro num">{group.papers.length}</span>
          </div>
          <p className="text-muted-foreground text-[13px] leading-[1.7]">{group.note}</p>

          <ol className="flex flex-col gap-5">
            {group.papers.map((paper) => (
              <li key={`${paper.authors}-${paper.venue}`} className="flex min-w-0 flex-col gap-1.5">
                <div className="flex flex-wrap items-baseline gap-x-2">
                  <span className="text-foreground text-[13px]">{paper.authors}</span>
                  {paper.title ? (
                    <span className="text-muted-foreground text-[13px]">{paper.title}</span>
                  ) : null}
                </div>
                <div className="flex flex-wrap items-baseline gap-x-3">
                  <span className="text-muted-foreground text-[12.5px]">{paper.venue}</span>
                  {paper.href ? (
                    <a
                      href={paper.href}
                      target="_blank"
                      rel="noreferrer"
                      className="num text-muted-foreground text-[11.5px] underline decoration-dotted underline-offset-4"
                    >
                      {paper.href.replace(/^https?:\/\/(doi\.org\/)?/, "")}
                    </a>
                  ) : null}
                </div>
                <p className="text-muted-foreground text-[13px] leading-[1.7]">{paper.uses}</p>
                <div className="flex flex-wrap gap-x-3">
                  {paper.cited.map((file) => (
                    // Not `.label-micro`: it uppercases, and an uppercased path is
                    // not the path. A reader is meant to be able to open these.
                    <span key={file} className="num text-muted-foreground/70 text-[11.5px]">
                      {file}
                    </span>
                  ))}
                </div>
              </li>
            ))}
          </ol>
        </div>
      ))}
    </Section>
  );
}
