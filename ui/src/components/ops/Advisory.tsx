/**
 * The AI advisory panel.
 *
 * MOCK: every value comes from `./data`. See that file.
 *
 * Provenance here is typographic and never chromatic: a dashed hairline border
 * and a `◊ INFERRED` tag, no colour on the panel at all. An operator must never
 * have to remember which hue meant "predicted"; the accent is spent on alarm and
 * nothing else. The panel distributes its rows evenly rather than stacking them
 * at the top, which is what leaves a dead gap above the risk figure.
 */

import { ADVISORY } from "@/components/ops/data";
import { fmt } from "@/lib/fmt";

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="min-w-0">
      <div className="label-micro">{label}</div>
      <div className="mt-[5px]">{children}</div>
    </div>
  );
}

function Divider() {
  return <div className="bg-border h-px shrink-0" aria-hidden="true" />;
}

export function Advisory() {
  return (
    <section className="marks relative flex min-h-0 flex-1 flex-col overflow-hidden">
      <div className="border-border flex h-9 shrink-0 items-center justify-between border-b px-4">
        <span className="t-section">AI ADVISORY</span>
        <span className="border-structure-hi text-muted-foreground border border-dashed px-[7px] py-[3px] text-[10px] tracking-[0.08em]">
          ◊ INFERRED
        </span>
      </div>

      <div className="flex min-h-0 flex-1 p-3">
        <div className="border-structure flex min-h-0 flex-1 flex-col justify-between gap-[10px] border border-dashed px-[14px] py-3">
          <Field label="Diagnosis">
            <span className="text-[13px] leading-[1.45] text-pretty">
              {ADVISORY.diagnosis}
              <span className="text-foreground-dim ml-2">confidence {ADVISORY.confidencePct}%</span>
            </span>
          </Field>

          <Divider />

          <Field label="Remaining useful life">
            <div className="flex items-baseline gap-3">
              <span className="num t-value">
                {fmt(ADVISORY.rulHours, 1)}
                <span className="text-muted-foreground ml-[5px] text-[11px]">h</span>
              </span>
              <span className="num text-muted-foreground text-[11px] leading-[1.4]">
                [{fmt(ADVISORY.rulLowHours, 1)} – {fmt(ADVISORY.rulHighHours, 1)}] @{" "}
                {ADVISORY.coveragePct}%
              </span>
            </div>
          </Field>

          <Divider />

          <Field label="Recommendation">
            <span className="text-[13px] leading-[1.45] text-pretty">
              {ADVISORY.recommendation}
            </span>
          </Field>

          <div className="border-border flex items-baseline justify-between border-t pt-[10px]">
            <span className="label-micro">Continue-mission risk</span>
            <span className="num text-[16px]">
              {ADVISORY.continueRiskPct}
              <span className="text-muted-foreground ml-[3px] text-[10px]">%</span>
            </span>
          </div>
        </div>
      </div>
    </section>
  );
}
