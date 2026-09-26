# Security policy

## Supported versions

| Version | Supported |
|---|---|
| 0.1.x (`main`) | Yes |

There are no earlier releases.

## Reporting a vulnerability

Email **64996768+mcp-tool-shop@users.noreply.github.com** with:

- what the vulnerability is;
- the steps that reproduce it;
- the commit or release it affects;
- what an attacker could do with it.

Please don't open a public issue for a vulnerability.

| Action | Target |
|---|---|
| Acknowledge the report | 48 hours |
| Assess its severity | 7 days |
| Release a fix | 30 days |

## What the code touches

- **The law** (`crates/law`, built to one WebAssembly module) does no I/O of any kind. CI checks that the
  module imports nothing.
- **The host** (the `host` binary) reads:
  - the score files under `scores/`;
  - the piano samples in its per-user cache;
  - the audio outputs and MIDI inputs you choose.

  It writes only two things: the files you name (`render`, `notes`, `preview`) and the per-user cache
  (`fetch-piano`). The cache is `%LOCALAPPDATA%\si-jam-sessions` on Windows, `~/Library/Caches/si-jam-sessions`
  on macOS, and `$XDG_CACHE_HOME/si-jam-sessions` or `~/.cache/si-jam-sessions` elsewhere. The piano's samples are in its `salamander-grand-piano-v3`
  folder.
- **The network** is used only by `host fetch-piano`. It runs the system's `curl` against one fixed HTTPS
  address, then checks the archive's size and SHA-256 against the values pinned in the source before it
  unpacks anything. The unpacker writes only regular files and directories under the archive's one top
  directory. It refuses links, absolute paths, paths that climb out of the directory, and bad headers. The
  piano opens only a cache whose marker names the pinned archive.
- **No telemetry.** Nothing is collected or sent. There are no accounts, and the code reads, stores and sends
  no credentials.

The review tools in `tools/review/` are maintainer tooling, not part of the product. They send review packets
to Ollama Cloud models through a local Ollama daemon, and CI never runs them.
