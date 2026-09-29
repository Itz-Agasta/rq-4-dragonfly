// usage: bun --env-file=<.env> recover.ts <outPrefix>   (reads <outPrefix>.req.json, free status reads only)
import { $ } from "bun";
const out = Bun.argv[2];
const sub = await Bun.file(`${out}.req.json`).json();
const H = { Authorization: `Key ${process.env.HF_CREDENTIALS}` };
let r: any = {};
for (let i = 0; i < 60; i++) {
  try {
    r = await (
      await fetch(sub.status_url, { headers: H, signal: AbortSignal.timeout(20000) })
    ).json();
  } catch (e) {
    console.log(`poll ${i} error`);
  }
  console.log(out.split("/").pop(), `poll ${i} ${r.status}`);
  if (["completed", "failed", "nsfw", "canceled"].includes(r.status)) break;
  await Bun.sleep(4000);
}
const urls = (r.images ?? []).map((x: any) => x.url).concat(r.video ? [r.video.url] : []);
for (const [k, u] of urls.entries()) {
  const f = `${out}_${k}.${u.split(".").pop().split("?")[0]}`;
  await $`curl -sS --fail --retry 3 --max-time 300 -o ${f} ${u}`;
  console.log("saved", f);
}
