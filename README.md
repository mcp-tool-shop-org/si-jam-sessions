<p align="center">
  <a href="README.md">English</a> | <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/si-jam-sessions/readme.png" alt="si-jam-sessions" width="400">
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/si-jam-sessions/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/si-jam-sessions/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
  <a href="https://mcp-tool-shop-org.github.io/si-jam-sessions/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page"></a>
</p>

# si-jam-sessions

**A music engine where an AI's playing can be graded exactly, replayed exactly, and trained on with a clean conscience.**

Play a phrase against the beat, and the engine records every note of the take:

- when it landed, in integer samples;
- which note of the score it answers.

It then grades each note against the score. Two machines grading the same take agree to the last bit, because
the grading law is deterministic Rust compiled to one pinned WebAssembly binary. The law is deterministic, so a recorded
take replays to the same hash without calling a model; CI replays one on every run.

`si-jam-sessions` is the counterpart to [`ai-jam-sessions`](https://github.com/mcp-tool-shop-org/ai-jam-sessions)
and a sibling of [`si-rpg-engine`](https://github.com/mcp-tool-shop-org/si-rpg-engine). The model proposes. A
checker that is not the model admits the proposal or refuses it with a reason, and the law commits and hashes
what it admits. The model never touches the score directly.

## Listen

Two arrangements of *Battle Hymn of the Republic* play side by side on the
[landing page](https://mcp-tool-shop-org.github.io/si-jam-sessions/), with a view of where they differ.

- **The source:** this project's transcription of the anonymous 1862 Ditson edition.
- **The arrangers:** glm-5.3 and kimi-k3, each working from that source on Ollama Cloud.
- **The licence:** both arrangements are dedicated CC0 1.0. Each passed the law's licence check with its
  evidence.
- **The recording:** each was rendered through the law on the
  [Salamander Grand Piano V3](https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html) by Alexander Holm
  (CC BY 3.0).

| Arrangement | Length | Notes the law committed | Uncompressed WAV (48 kHz, 32-bit float) |
|---|---|---|---|
| glm-5.3 | 5:02 | 1,720 | [116 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-glm-5.3.wav), SHA-256 `85b2d567…2eed` |
| kimi-k3 | 4:37 | 1,924 | [106 MB](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/download/v0.1.0/battle-hymn-kimi-k3.wav), SHA-256 `bf49d310…a017` |

Rendering is deterministic: on the Windows machine that made them, a second render gives the same bytes. A
render on another platform has not been compared yet. After fetching the piano once, this command reproduces
the first one:

```bash
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
```

The full hashes are in the [release notes](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0).

## What makes it different

- **The take is part of the law.** Timing, pitch and velocity are integers in the hashed state, so "late by
  45 ms" is a fact the engine can prove. The waveform is only presentation.
- **The clock never waits.** The law steps a fixed quantum whether anyone plays or not. A model plans phrases
  ahead of a commit horizon. A late plan is refused, never allowed to stall the music.
- **Every song earns its place.** Music enters only with evidence:
  - a composition in the public domain in both the US and the EU;
  - an arrangement in the public domain, or one this project engraved;
  - the source edition and its year;
  - an in-file licence that matches its receipt.

  Unknown, share-alike, non-commercial and AI-restricted sources are refused. CC BY 4.0 material sits in its
  own attributed tier.
- **Training data will be a printout of what the law committed.** No dataset has been built yet. When one is,
  every row will replay to the same hash, splits will be by work and fixed before any row exists, and every
  claimed result will state its statistical power.

## Try it

You need Rust. The repository pins version 1.98.1 in `rust-toolchain.toml`, and `rustup` installs it on the
first build.

- **Windows 10 and 11** run everything, though live input has not yet been tried with a real MIDI keyboard.
- **Linux:** CI builds and tests the host and renders with it. Playing through a Linux audio device is
  untested, and live input is Windows-only for now.
- **macOS** is untested.

```bash
cargo run -p host --release -- devices        # list the audio outputs and MIDI inputs
cargo run -p host --release -- fetch-piano    # download and verify the grand piano once (742 MB)
cargo run -p host --release -- play           # the Battle Hymn as glm-5.3 arranged it
cargo run -p host --release -- play --piece battle-hymn-kimi-k3
cargo run -p host --release -- jam --midi 0   # play along; each note is graded as it lands
```

- `--piece` picks `battle-hymn-glm-5.3` (the default), `battle-hymn-kimi-k3`, or `entertainer`, the first
  milestone's test piece.
- `notes <out.json>` writes the notes the law commits. The landing page draws from these files.
- `host help` lists every command.

Use a wired output for `jam`. A Bluetooth output's delay is longer than the 100 ms the law allows for delivery.

## Trust model

- **Data touched:**
  - the score files in `scores/`;
  - the piano samples in a per-user cache;
  - the audio outputs and MIDI inputs you choose;
  - the files you ask it to write.
- **Data not touched:** anything outside those paths. There are no accounts, no credentials and no telemetry.
- **Network:** only `fetch-piano` uses it.
  - It downloads one archive from one fixed address.
  - It checks the archive against a pinned SHA-256 before unpacking anything.
  - It refuses links, and paths that climb out of its directory.
- **Permissions:** an ordinary user account. Nothing needs administrator rights.
- **The law** does no I/O at all. CI checks that its WebAssembly module imports nothing.

To report a vulnerability, see [`SECURITY.md`](SECURITY.md).

## Where it stands

- **The first milestone is complete:** the law, ingest and provenance, the golden hash, and the host. The
  golden hash is the same natively on x86_64 and ARM64, and as WebAssembly under V8, SpiderMonkey and
  JavaScriptCore.
- **The design** is locked in [`docs/PHASE-0.md`](docs/PHASE-0.md).
- **The handoff** in [`docs/HANDOFF.md`](docs/HANDOFF.md) covers what is in flight, what was decided and why,
  and what comes next.
- **Review:** before a code change merges, two models review it on Ollama Cloud. Each comes from a family other
  than the author's, and each is blind to the other's review.

## License

- **Code:** MIT (see [`LICENSE`](LICENSE)).
- **The two Battle Hymn arrangements:** CC0 1.0.
- **The piano samples:** CC BY 3.0 (Alexander Holm). `fetch-piano` downloads them, and they are never
  committed.
- **Datasets:** each carries its own licence on its own card.

---

Built by <a href="https://mcp-tool-shop.github.io/">MCP Tool Shop</a>
