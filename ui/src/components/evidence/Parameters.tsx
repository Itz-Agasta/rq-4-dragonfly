/**
 * 02 PARAMETERS. Every constant the model runs on, and which ones are attackable.
 *
 * The filter defaults to showing everything. A reviewer who wants to attack a
 * number wants the estimated set, so that is one click, and the proportion bar
 * above states the split before anyone has to count rows.
 */

import { useState } from "react";

import {
  ESTIMATED_COUNT,
  PARAMETERS,
  PUBLISHED_COUNT,
  SECTIONS,
} from "@/components/evidence/data/parameters";
import { Cell, P, Quote, Row, Section, Stats, Table, Tag } from "@/components/evidence/parts";

type Filter = "all" | "published" | "estimated";

const TOTAL = PARAMETERS.length;

function Controls({ value, onChange }: { value: Filter; onChange: (f: Filter) => void }) {
  const options: [Filter, string][] = [
    ["all", `ALL ${TOTAL}`],
    ["published", `PUBLISHED ${PUBLISHED_COUNT}`],
    ["estimated", `ESTIMATED ${ESTIMATED_COUNT}`],
  ];
  return (
    <div className="border-border flex w-fit border">
      {options.map(([key, label]) => (
        <button
          key={key}
          type="button"
          onClick={() => onChange(key)}
          className={`label-micro border-border px-3 py-1.5 transition-colors not-last:border-r ${
            value === key
              ? "bg-popover text-foreground"
              : "text-structure hover:bg-popover hover:text-muted-foreground"
          }`}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

export function Parameters() {
  const [filter, setFilter] = useState<Filter>("all");
  const rows = PARAMETERS.filter((p) => filter === "all" || p.source === filter);

  return (
    <Section
      id="parameters"
      index="02"
      title="Every constant and its source"
      standfirst={`The model runs on ${TOTAL} constants. The parameter file records a source for every one of them, marked either published or estimated.`}
    >
      <Stats
        items={[
          [`${TOTAL}`, "constants the model runs on"],
          [`${PUBLISHED_COUNT}`, "published by a certificate or a standard"],
          [`${ESTIMATED_COUNT}`, "estimated, then fitted to the rating point"],
        ]}
      />

      <P>
        Published means a type certificate, a manufacturer factsheet or a materials standard states
        the figure. Estimated means nobody publishes it: the value comes from engine class, fitted
        until the model reproduces the single published rating point. No figure here was measured on
        this engine, and the parameter file header says so in its opening paragraph.
      </P>

      <Quote>
        The estimated set is the part of this model open to dispute. It is one click away.
      </Quote>

      <P>
        Volumetric efficiency shows what that buys. It is three coefficients, not a surface fitted
        to data this engine has never produced. A propulsion engineer can dispute three numbers.
      </P>

      <Controls value={filter} onChange={setFilter} />

      <Table
        columns={["Section", "Parameter", "Value", "Source", "Note"]}
        align={["l", "l", "r", "l", "l"]}
      >
        {rows.map((p) => (
          <Row key={`${p.section}.${p.key}`}>
            <Cell dim>{p.section}</Cell>
            <Cell>{p.key}</Cell>
            <Cell numeric>{p.value}</Cell>
            <Cell>
              <Tag kind={p.source} />
            </Cell>
            <Cell dim wide>
              {p.note ?? "·"}
            </Cell>
          </Row>
        ))}
      </Table>

      <span className="label-micro">
        {rows.length} of {TOTAL} shown · {SECTIONS.length} tables ·
        crates/engine-model/src/engines/ae330.toml
      </span>
    </Section>
  );
}
