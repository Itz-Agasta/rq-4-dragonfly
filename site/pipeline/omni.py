"""Gemini Omni 1.1 Flash image-to-video over REST, stdlib only.
usage: omni.py <out.mp4> <resolution> <prompt> <first.jpg> [last.jpg]
STORE=1 keeps the interaction so it can be upscaled or extended later via previous_interaction_id.
Key read from GOOGLE_CREDENTIALS in the repo .env. Synchronous unary call (background=false,
store=false per the Omni docs' fast path); the video comes back base64 in steps[].content[].
https://ai.google.dev/gemini-api/docs/omni
"""
import base64, json, os, pathlib, sys, time, urllib.request

out, res, prompt, *frames = sys.argv[1:]
env = pathlib.Path(__file__).resolve().parents[2] / ".env"
key = next(l.split("=", 1)[1].strip().strip("\"'") for l in env.read_text().splitlines() if l.startswith("GOOGLE_CREDENTIALS="))
inp = [{"type": "image", "data": base64.b64encode(pathlib.Path(f).read_bytes()).decode(), "mime_type": "image/jpeg"} for f in frames]
inp.append({"type": "text", "text": prompt})
body = {"model": "gemini-omni-1.1-flash", "input": inp, "background": False, "store": os.environ.get("STORE") == "1",
        "response_format": {"type": "video", "resolution": res, "aspect_ratio": "16:9"}}
if len(frames) == 1:
    body["generation_config"] = {"video_config": {"task": "image_to_video"}}
req = urllib.request.Request("https://generativelanguage.googleapis.com/v1beta/interactions",
                             data=json.dumps(body).encode(), headers={"Content-Type": "application/json", "x-goog-api-key": key})
t = time.time(); print("submitted", flush=True)
try:
    d = json.load(urllib.request.urlopen(req, timeout=900))
except urllib.error.HTTPError as e:
    print("http", e.code, e.read()[:600].decode(errors="replace")); sys.exit(1)
vid = [c for s in d.get("steps", []) if s.get("type") == "model_output" for c in s.get("content", []) if c.get("type") == "video"]
if not vid:
    print("no video", json.dumps(d)[:600]); sys.exit(1)
pathlib.Path(out).write_bytes(base64.b64decode(vid[0]["data"]))
pathlib.Path(out + ".id").write_text(d.get("id", ""))
print(f"saved {out} in {time.time() - t:.0f}s status={d.get('status')}")
