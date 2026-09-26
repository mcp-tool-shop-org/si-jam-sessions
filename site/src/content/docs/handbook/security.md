---
title: Security
description: What si-jam-sessions touches, what it never does, and how to report a vulnerability.
sidebar:
  order: 6
---

## The trust model

**The law does no I/O of any kind.** It is one WebAssembly module, and CI checks that it imports nothing. It
cannot read a file, open a socket or look at the clock. The host hands it bytes and reads back frames.

**The host** reads four things:

- the score files under `scores/`;
- the piano samples in its per-user cache;
- the audio outputs you choose;
- the MIDI inputs you choose.

It writes two:

- the files you name, for `render`, `notes` and `preview`;
- the per-user cache, for `fetch-piano`.

| System | Piano cache |
|---|---|
| Windows | `%LOCALAPPDATA%\si-jam-sessions` |
| macOS | `~/Library/Caches/si-jam-sessions` |
| Linux and others | `$XDG_CACHE_HOME/si-jam-sessions`, or `~/.cache/si-jam-sessions` |

The piano's samples are in the cache's `salamander-grand-piano-v3` folder.

`render`, `notes` and `preview` replace a file that already exists at the path you give. A later release may
ask first.

**The network is used only by `fetch-piano`.**

- It runs the system's `curl` against one fixed HTTPS address. The host links no TLS library of its own.
- The archive's size and SHA-256 must equal the values pinned in the source, or the download is deleted and
  nothing is unpacked.
- The unpacker writes only regular files and directories under the archive's one top directory. It refuses
  links, absolute paths, paths that climb out of the directory, names outside the set's characters, and headers
  whose checksum is wrong.
- A marker naming the archive's SHA-256 is written last. The piano opens only a cache that carries it, so a
  fetch that stopped partway is never played.

**No telemetry.** Nothing is collected or sent. There are no accounts, and the code reads, stores and sends no
credentials.

**Permissions.** An ordinary user account is enough. Nothing needs administrator rights.

## The audio callback

The audio callback never allocates, locks, blocks or does I/O. A counting allocator in the tests proves that it
allocates nothing. A slow or misbehaving device therefore cannot make the host corrupt what the law committed.
The host plays the law's frames and feeds nothing back into the law.

## Maintainer tooling

The review tools in `tools/review/` send review packets to Ollama Cloud models through a local Ollama daemon.
They are not part of the product, and CI never runs them.

## Reporting a vulnerability

Email **64996768+mcp-tool-shop@users.noreply.github.com**. Say what the vulnerability is, how to reproduce it,
which commit or release it affects, and what an attacker could do with it. Please don't open a public issue for
a vulnerability. The targets are to acknowledge a report within 48 hours, assess it within 7 days, and release
a fix within 30 days.
