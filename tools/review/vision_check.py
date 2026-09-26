"""Ask a vision-capable Ollama Cloud model to check a transcription against page scans; record the call.

  py -3.14 vision_check.py --model kimi-k3:cloud --brief BRIEF.md --text FILE --out PREFIX IMG [IMG ...]
"""
import argparse
import base64
import hashlib
import io
import json
import pathlib
import time
import urllib.request

from PIL import Image

ap = argparse.ArgumentParser()
ap.add_argument("--model", required=True)
ap.add_argument("--think", default="high")
ap.add_argument("--brief", required=True)
ap.add_argument("--text", required=True)
ap.add_argument("--out", required=True)
ap.add_argument("--width", type=int, default=2200)
ap.add_argument("images", nargs="+")
a = ap.parse_args()

imgs, meta_imgs = [], []
for p in a.images:
    raw = pathlib.Path(p).read_bytes()
    im = Image.open(io.BytesIO(raw)).convert("L")
    if im.width > a.width:
        im = im.resize((a.width, round(im.height * a.width / im.width)), Image.LANCZOS)
    buf = io.BytesIO()
    im.save(buf, "JPEG", quality=90)
    imgs.append(base64.b64encode(buf.getvalue()).decode())
    meta_imgs.append({"path": p, "sha256": hashlib.sha256(raw).hexdigest(), "sent_px": [im.width, im.height]})

brief = pathlib.Path(a.brief).read_text(encoding="utf-8")
text = pathlib.Path(a.text).read_text(encoding="utf-8")
content = f"{brief}\n\n===== FILE: {pathlib.Path(a.text).name} =====\n{text}\n\nThe page images follow, in order: " + \
    ", ".join(pathlib.Path(p).name for p in a.images)
body = {"model": a.model, "stream": True, "think": a.think, "options": {"temperature": 0},
        "messages": [{"role": "user", "content": content, "images": imgs}]}
t0 = time.time()
out, think, final = [], [], {}
req = urllib.request.Request("http://localhost:11434/api/chat", data=json.dumps(body).encode(),
                             headers={"Content-Type": "application/json"})
with urllib.request.urlopen(req, timeout=900) as r:
    for line in r:
        if line.strip():
            d = json.loads(line)
            if "error" in d:
                raise SystemExit(f"STOP: {d['error']}")
            m = d.get("message", {})
            out.append(m.get("content") or "")
            think.append(m.get("thinking") or "")
            if d.get("done"):
                final = d
pathlib.Path(a.out + ".md").write_text("".join(out).strip() + "\n", encoding="utf-8")
pathlib.Path(a.out + ".thinking.txt").write_text("".join(think), encoding="utf-8")
meta = {"model_requested": a.model, "model_served": final.get("model"), "think": a.think,
        "brief_sha256": hashlib.sha256(brief.encode()).hexdigest(),
        "text_sha256": hashlib.sha256(text.encode()).hexdigest(), "images": meta_imgs,
        "prompt_eval_count": final.get("prompt_eval_count"), "eval_count": final.get("eval_count"),
        "done_reason": final.get("done_reason"), "wall_s": round(time.time() - t0, 1)}
pathlib.Path(a.out + ".meta.json").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
print(a.out, meta["model_served"], meta["prompt_eval_count"], meta["eval_count"], meta["done_reason"], meta["wall_s"], "s")
