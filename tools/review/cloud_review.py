"""Run an external review on an Ollama Cloud model through the local daemon, and record the call.

  py -3.14 cloud_review.py --model kimi-k3:cloud --brief BRIEF.md --out PREFIX FILE [FILE ...]

- Sends the brief and every FILE verbatim (each under a header naming it), with thinking raised
  (--think, default "high"), streaming so a long review never idles a connection.
- Writes PREFIX.md (the review), PREFIX.thinking.txt (the model's reasoning) and PREFIX.meta.json
  (model asked for and served, think level, sha256 of the prompt and of each file, token counts,
  durations). Never overwrites: an existing PREFIX gets -2, -3, ...
"""
import argparse
import hashlib
import json
import pathlib
import sys
import time
import urllib.request

URL = "http://localhost:11434/api/chat"
SYSTEM = (
    "You are an external code reviewer from a different model family than the authors. You see only "
    "the change, its specification and its evidence, never the authors' reasoning. You cannot run "
    "anything: reason from the text you are given. Do not assume the contents of files you were not "
    "given; if a check needs them, say so under DID NOT CHECK. Every finding names a file and line "
    "(from the patch hunks), what is wrong, why it matters, and the smallest fix. Say 'no findings' "
    "when a packet is sound. Follow the brief's output format exactly."
)

ap = argparse.ArgumentParser()
ap.add_argument("--model", required=True)
ap.add_argument("--think", default="high")
ap.add_argument("--brief", required=True)
ap.add_argument("--out", required=True)
ap.add_argument("--read-timeout", type=int, default=900, help="seconds allowed between streamed chunks")
ap.add_argument("--num-predict", type=int, default=262144, help="output token cap, thinking included")
ap.add_argument("--system", help="file whose text replaces the reviewer system prompt")
ap.add_argument("files", nargs="*")
a = ap.parse_args()


def sha(b):
    return hashlib.sha256(b).hexdigest()


brief = pathlib.Path(a.brief).read_bytes()
parts = [brief.decode("utf-8")]
files_meta = []
for f in a.files:
    b = pathlib.Path(f).read_bytes()
    files_meta.append({"path": f, "bytes": len(b), "sha256": sha(b)})
    parts.append(f"\n\n===== FILE: {pathlib.Path(f).name} ({len(b)} bytes) =====\n{b.decode('utf-8', 'replace')}")
user = "".join(parts)
system_text = pathlib.Path(a.system).read_text(encoding="utf-8") if a.system else SYSTEM
messages = [{"role": "system", "content": system_text}, {"role": "user", "content": user}]
prompt_sha = sha(json.dumps(messages, ensure_ascii=False).encode("utf-8"))

out = pathlib.Path(a.out)
n = 1
while any(pathlib.Path(f"{out}{'' if n == 1 else f'-{n}'}{ext}").exists() for ext in (".md", ".meta.json")):
    n += 1
stem = f"{out}{'' if n == 1 else f'-{n}'}"

body = {"model": a.model, "messages": messages, "stream": True, "think": a.think, "options": {"temperature": 0, "num_predict": a.num_predict}}
req = urllib.request.Request(URL, data=json.dumps(body).encode("utf-8"), headers={"Content-Type": "application/json"})
t0 = time.time()
content, thinking, final = [], [], {}
with urllib.request.urlopen(req, timeout=a.read_timeout) as r:
    for line in r:
        if not line.strip():
            continue
        d = json.loads(line)
        if "error" in d:
            sys.exit(f"STOP: {d['error']}")
        m = d.get("message", {})
        if m.get("content"):
            content.append(m["content"])
        if m.get("thinking"):
            thinking.append(m["thinking"])
        if d.get("done"):
            final = d
elapsed = time.time() - t0

text = "".join(content).strip()
pathlib.Path(stem + ".md").write_text(text + "\n", encoding="utf-8")
pathlib.Path(stem + ".thinking.txt").write_text("".join(thinking), encoding="utf-8")
meta = {
    "model_requested": a.model,
    "model_served": final.get("model"),
    "think": a.think,
    "temperature": 0,
    "num_predict": a.num_predict,
    "prompt_sha256": prompt_sha,
    "brief": {"path": a.brief, "sha256": sha(brief)},
    "files": files_meta,
    "prompt_eval_count": final.get("prompt_eval_count"),
    "eval_count": final.get("eval_count"),
    "done_reason": final.get("done_reason"),
    "total_duration_s": (final.get("total_duration") or 0) / 1e9,
    "wall_s": round(elapsed, 1),
    "started": time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime(t0)),
    "thinking_chars": sum(map(len, thinking)),
    "answer_chars": len(text),
}
pathlib.Path(stem + ".meta.json").write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
if not text:
    print(f"EMPTY ANSWER: done_reason={final.get('done_reason')}; raise --num-predict or lower --think")
print(f"{stem}.md  served={meta['model_served']} prompt={meta['prompt_eval_count']} out={meta['eval_count']} "
      f"thinking={meta['thinking_chars']} wall={meta['wall_s']}s done={meta['done_reason']}")
