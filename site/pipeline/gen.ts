// usage: bun --env-file=<.env> gen.ts <model> <outPrefix> <jsonBodyFile>
import { $ } from "bun";
const [model, out, bodyFile] = Bun.argv.slice(2);
const H = {
  Authorization: `Key ${process.env.HF_CREDENTIALS}`,
  "Content-Type": "application/json",
};
const body = await Bun.file(bodyFile).text();
const est = await fetch(`https://api.higgsfield.ai/estimate/${model}`, {
  method: "POST",
  headers: H,
  body,
  signal: AbortSignal.timeout(20000),
});
console.log("estimate", await est.text());
const sub = await (
  await fetch(`https://api.higgsfield.ai/${model}`, {
    method: "POST",
    headers: H,
    body,
    signal: AbortSignal.timeout(30000),
  })
).json();
console.log("submitted", sub.request_id, sub.status);
await Bun.write(`${out}.req.json`, JSON.stringify(sub));
let r: any = sub;
for (let i = 0; i < 300 && !["completed", "failed", "nsfw", "canceled"].includes(r.status); i++) {
  await Bun.sleep(4000);
  try {
    r = await (
      await fetch(sub.status_url, { headers: H, signal: AbortSignal.timeout(20000) })
    ).json();
  } catch (e) {
    console.log(`poll ${i} error ${e}`);
    continue;
  }
  console.log(`poll ${i} ${r.status}`);
}
if (r.status !== "completed") {
  console.log(JSON.stringify(r));
  process.exit(1);
}
const urls = (r.images ?? []).map((x: any) => x.url).concat(r.video ? [r.video.url] : []);
for (const [k, u] of urls.entries()) {
  const f = `${out}_${k}.${u.split(".").pop().split("?")[0]}`;
  await $`curl -sS --fail --retry 3 --max-time 300 -o ${f} ${u}`;
  console.log("saved", f, u);
}
