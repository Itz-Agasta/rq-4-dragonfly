/**
 * Shared furniture for the EVIDENCE document.
 *
 * **This is the one screen designed to be read rather than watched**, so it gets
 * an editorial scale the six operational screens do not: 28px section openers,
 * a 17px lead, 14px body, 40px figures and 20px pull quotes. Hierarchy comes from
 * size and space rather than from a second typeface, because Geist Mono is the
 * product's voice and a serif here would read as a web page pasted into an
 * instrument. Set at the UI's own 13 to 16px the page is one undifferentiated
 * grey wall and a reader scrolling it has nowhere to land.
 *
 * **The accent has exactly three roles here:** the eyebrow number on a section,
 * the rule beside a pull quote, and links. Always those, never anything else, so
 * the page reads as designed rather than decorated. `--primary` means "needs
 * attention" on every other screen and the nav rail refuses it even for "you are
 * here"; this screen carries no live value and no alarm for it to compete with.
 *
 * **Two things stay deliberately colourless.** Provenance tags are a dashed
 * hairline and a word, never a hue, because a reader must never learn to read
 * colour as certainty. And a delta like `+1.7%` stays neutral: in orange it
 * reads as a fault rather than as a result somebody measured and published.
 *
 * The screen is a document, which nothing else in this app is, so it needs a
 * small vocabulary of its own: a numbered section, a prose column, a definition
 * row and a table. Everything here stays inside the lattice grammar the rest of
 * the interface uses, because a document panel that invents its own borders
 * reads as a web page pasted into an instrument.
 *
 * **Nothing is capped to a reading measure.** Prose, tables and the lattice
 * blocks all run the full pane; only figures stop, at their own 1100px, and are
 * centred. A typographic measure was tried at 72 characters and rejected: on a
 * dark instrument panel the reserved space reads as a page that failed to load
 * rather than as a considered column, and this screen is shown on a projector.
 */

import type React from "react";

/**
 * Running text on this screen.
 *
 * 14px at 1.7, not the app's 13px at 1.45. The rest of the interface is read in
 * glances at a fixed distance; this screen is read in paragraphs, sometimes from
 * a projector, and the lines run the full pane. Long lines need the extra leading
 * to stop the eye landing on the wrong one at the wrap.
 */
export const READING = "text-[14px] leading-[1.7]";

/**
 * A section opener: eyebrow, title, lead.
 *
 * Sentence case, not the uppercase the rest of the app uses. An uppercase 16px
 * string reads as a control label; this is a chapter heading and has to announce
 * itself as one. The eyebrow rule runs the full width so the reader sees the
 * break while scrolling past at speed, before any word is legible.
 */
export function Section({
  id,
  index,
  title,
  standfirst,
  children,
}: {
  id: string;
  index: string;
  title: string;
  /** The lead. One or two sentences, set larger and brighter than the body. */
  standfirst?: string;
  children: React.ReactNode;
}) {
  return (
    <section id={id} className="scroll-mt-4">
      <header className="mb-8">
        <div className="mb-5 flex items-center gap-4">
          <span className="label-micro num text-[var(--primary)]">{index}</span>
          <span className="h-px flex-1 bg-[var(--primary)]/35" />
        </div>
        <h2 className="text-foreground text-[28px] leading-[1.15] tracking-tight">{title}</h2>
        {standfirst ? (
          <p className={`text-foreground mt-4 text-[17px] leading-[1.55]`}>{standfirst}</p>
        ) : null}
      </header>
      <div className="flex flex-col gap-7">{children}</div>
    </section>
  );
}

/**
 * A pull quote: the one sentence in a section worth stopping for.
 *
 * Replaces the labelled callout wherever the point is a claim rather than an
 * aside. A reader skimming five sections should be able to read only these and
 * come away with the argument.
 */
export function Quote({ children }: { children: React.ReactNode }) {
  return (
    <blockquote className={`my-1 border-l-2 border-[var(--primary)] py-1 pl-6`}>
      <p className="text-foreground text-[20px] leading-[1.45] tracking-tight">{children}</p>
    </blockquote>
  );
}

/**
 * A row of headline figures.
 *
 * The strongest entry point on a scroll, and on this screen the numbers are the
 * argument rather than an illustration of it. Deliberately not accented: five
 * large orange numbers per screen reads as an alarm board, which is the one
 * thing this product's colour must not be spent on.
 */
export function Stats({ items }: { items: [string, string][] }) {
  return (
    <div
      className="lattice"
      style={{ gridTemplateColumns: `repeat(${items.length}, minmax(0, 1fr))` }}
    >
      {items.map(([value, label]) => (
        <section key={label} className="cell flex flex-col gap-2 py-5">
          <span className="num text-foreground text-[40px] leading-none tracking-tight">
            {value}
          </span>
          <span className="label-micro">{label}</span>
        </section>
      ))}
    </div>
  );
}

/** An external reference. One of the accent's three jobs. */
export function Link({ href, children }: { href: string; children: React.ReactNode }) {
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      className="text-[var(--primary)] underline decoration-dotted underline-offset-4"
    >
      {children}
    </a>
  );
}

/** A body paragraph. */
export function P({ children }: { children: React.ReactNode }) {
  return <p className={`${READING} text-muted-foreground`}>{children}</p>;
}

/**
 * A labelled aside: a caveat, a definition, a note on method.
 *
 * Kept structurally distinct from [`Quote`]. A quote is the argument and takes
 * the accent; a callout is a supporting note and takes a grey rule, or the two
 * compete and neither is emphasis any more.
 */
export function Callout({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className={`border-structure-hi border-l py-2 pl-5`}>
      <span className="label-micro block">{label}</span>
      <div className={`${READING} text-foreground mt-2`}>{children}</div>
    </div>
  );
}

/**
 * A dense data table.
 *
 * `overflow-x-auto` on the wrapper, never on the page: the body must not scroll
 * sideways at any viewport this ships to.
 */
export function Table({
  columns,
  align,
  children,
}: {
  columns: string[];
  /** Per column. Numerics right, everything else left. */
  align?: ("l" | "r")[];
  children: React.ReactNode;
}) {
  return (
    <div className="border-border min-w-0 overflow-x-auto border">
      <table className="w-full border-collapse text-left">
        <thead>
          <tr className="border-border border-b">
            {columns.map((c, i) => (
              <th
                key={c}
                className={`label-micro px-3 py-2.5 font-normal ${
                  align?.[i] === "r" ? "text-right" : ""
                }`}
              >
                {c}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>{children}</tbody>
      </table>
    </div>
  );
}

export function Row({ children }: { children: React.ReactNode }) {
  return <tr className="border-border/60 border-b last:border-b-0">{children}</tr>;
}

export function Cell({
  children,
  numeric,
  dim,
  wide,
}: {
  children: React.ReactNode;
  numeric?: boolean;
  dim?: boolean;
  wide?: boolean;
}) {
  return (
    <td
      className={`px-3 py-2.5 align-top text-[12.5px] leading-[1.6] ${
        numeric ? "num text-right" : ""
      } ${dim ? "text-muted-foreground" : "text-foreground"}`}
    >
      {/* `min-w` only, no cap: a note column that collapses to two words per line
          is worse than a long one. `td` ignores width under auto table layout, so
          it goes on a block child. */}
      {wide ? <div className="min-w-[280px]">{children}</div> : children}
    </td>
  );
}

/**
 * The provenance tag.
 *
 * Typographic, never chromatic, which is the standing rule in this app: an
 * operator must never have to tell a measurement from a prediction by hue.
 * `published` is set in full luminance, `estimated` is dimmed and dashed.
 */
export function Tag({ kind }: { kind: "published" | "estimated" }) {
  return kind === "published" ? (
    <span className="label-micro text-foreground border-structure-hi border px-1.5 py-0.5">
      PUBLISHED
    </span>
  ) : (
    <span className="label-micro text-foreground-dim border-foreground-dim border border-dashed px-1.5 py-0.5">
      ESTIMATED
    </span>
  );
}

/** A term and its expansion, used where a table would be three columns of prose. */
export function Defs({ items }: { items: [string, string][] }) {
  return (
    <dl className="flex flex-col gap-4">
      {items.map(([term, body]) => (
        <div key={term}>
          <dt className="label-micro text-foreground">{term}</dt>
          <dd className={`${READING} text-muted-foreground mt-1.5`}>{body}</dd>
        </div>
      ))}
    </dl>
  );
}
