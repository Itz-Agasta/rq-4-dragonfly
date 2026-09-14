// Fails `just check` on a tour step that outgrew a tooltip.
//
// The first draft of `steps.ts` averaged 55 words a step and ran to 90, which
// reads as a help article in a popover and gets dismissed rather than read.
// Chameleon's benchmark over 550M in-app interactions puts the working limits
// near 25 words and 180 characters. Three rewrites later the same file still
// had three steps over, which is why this is a check and not a note in a doc.
//
// Node imports the TypeScript directly (type stripping, on by default since
// v22.18), so there is no parser here and nothing to drift out of step with
// `steps.ts`. That works only because the file's single import is `import
// type`, which strips to nothing; a value import would need a resolver.
//
// https://www.chameleon.io/blog/onboarding-ux-patterns

import { TOUR } from "../src/lib/tour/steps.ts";

const MAX_WORDS = 25;
const MAX_CHARS = 180;
const MAX_TITLE_WORDS = 8;
// A legend is scanned a row at a time, so it is budgeted by rows, not sentences.
const MAX_LEGEND_ROWS = 8;
const MAX_LEGEND_WORDS = 7;

const text = (html) =>
  html
    .replace(/<[^>]+>/g, " ")
    .replace(/\s+/g, " ")
    .trim();
const count = (s) => (s ? s.split(" ").length : 0);

const failures = [];

for (const { title, body } of TOUR) {
  const prose = text(body.replace(/<ul[\s\S]*?<\/ul>/g, ""));
  if (count(prose) > MAX_WORDS) failures.push(`${title}: ${count(prose)} words > ${MAX_WORDS}`);
  if (prose.length > MAX_CHARS) failures.push(`${title}: ${prose.length} chars > ${MAX_CHARS}`);
  if (count(title) > MAX_TITLE_WORDS)
    failures.push(`${title}: title over ${MAX_TITLE_WORDS} words`);

  const rows = [...body.matchAll(/<li>(.*?)<\/li>/g)].map((m) => text(m[1]));
  if (rows.length > MAX_LEGEND_ROWS) failures.push(`${title}: ${rows.length} legend rows`);
  for (const row of rows) {
    if (count(row) > MAX_LEGEND_WORDS) failures.push(`${title}: legend row "${row}" too long`);
  }
}

if (failures.length > 0) {
  console.error(`tour-lint: ${failures.length} over budget\n`);
  for (const line of failures) console.error(`  ${line}`);
  console.error("\nSplit the step or cut the second fact. A tour step is scanned, not read.");
  process.exit(1);
}

console.log(`tour-lint: ${TOUR.length} steps within ${MAX_WORDS} words / ${MAX_CHARS} chars`);
