# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/). It stays below 1.0 until the product is 1.0.

## [0.2.0] - 2026-09-28

Predicate version 4, a container, and the npm package. The committed notes did not move. The golden digests
did, because the snapshot header carries the predicate's version.

### Added

- **Licence predicate version 4**, still in law version 5. Terms that limit scraping, crawling, spidering,
  harvesting, bots, or automated access or retrieval are refused as their own class. The phrases match as
  whole words.
- **Two curated admissions.** The texts "no known copyright restrictions" and "no copyright - united states"
  are public domain when the text is exactly that wording. A longer sentence that contains one of them still
  negates.
- **CC0's SPDX forms** are admitted: `CC0-1.0` and `Creative Commons Zero v1.0 Universal`.
- **The container** `ghcr.io/mcp-tool-shop-org/si-jam-sessions`. It holds the host and the scores. The piano
  stays a checked download. The process drops privileges before the host runs.
- **The npm package** `@si-jam-sessions/si-jam-sessions`, the release record (this file, the README and the
  licence), published by Trusted Publishing from `release.yml`. It does not install the host.

### Changed

- The exemplar goldens are checked under each pinned JavaScript engine, and `notes`, `render` and `preview`
  refuse to overwrite a file that is already there.
- The full mix (score piano, take oscillator and click) renders to the same bits on Linux and Windows, and CI
  checks that pin.
- In piano mode the take and live notes stay on the oscillator. The help text and the handbook say so.

### Fixed

- A held MIDI key is released on every poll while the port count is low, instead of being latched.
- The unpacked piano samples' trust boundary is stated where the fetch unpacks them.

The Entertainer's golden is `b43db3787577b4f959f611ae6e7d84c0b92e69d34c31bcbfe927a9f67f0eda11`. The exemplar
goldens are glm-5.3 `409a3341a35af9e7ad203ef573a28e475600b27e183cc08d1b8b0cfbb03d2f97` and kimi-k3
`e792361b22413de14e1cd0740947af2a34725970ef0d126be1e720f96b06395f`. The recordings on
[v0.1.0](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0) are unchanged: the frames
the law commits are the same bytes.

## [0.1.0] - 2026-09-26

The first release. Its assets are the two Battle Hymn renders as uncompressed WAV files.

### Added

- **The law** ([#1](https://github.com/mcp-tool-shop-org/si-jam-sessions/pull/1)):
  - integer time at 3,360 ticks per quarter note;
  - a quantum of 48 samples at 48 kHz, and a commit horizon of 100 quanta;
  - an inclusive grading gate of ±1,920 samples (±40 ms);
  - a hashed little-endian snapshot and a raw C ABI;
  - `no_std`, with no floating point anywhere in the WebAssembly build.
- **Ingest and provenance**
  ([#2](https://github.com/mcp-tool-shop-org/si-jam-sessions/pull/2)):
  - Standard MIDI files are read strictly;
  - a licence predicate admits or refuses each score, with a reason;
  - *The Entertainer* is admitted with its receipt.
- **The golden hash** ([#4](https://github.com/mcp-tool-shop-org/si-jam-sessions/pull/4)): a constructed take
  of *The Entertainer* is graded and hashed. The hash is the same natively on x86_64 and ARM64, and as
  WebAssembly under V8, SpiderMonkey and JavaScriptCore, with each engine pinned by the SHA-256 of every file
  it runs.
- **Committed frames and live notes**
  ([#5](https://github.com/mcp-tool-shop-org/si-jam-sessions/pull/5)): a read-only export of what the law
  committed, and a verb for notes played live.
- **The host** ([#6](https://github.com/mcp-tool-shop-org/si-jam-sessions/pull/6)):
  - `devices`, `play`, `render`, `jam` and `jitter`;
  - audio output through cpal, with a callback that never allocates;
  - MIDI input through WinMM, and a computer-keyboard fallback.
- **Licence predicate version 3, in law version 5**
  ([#10](https://github.com/mcp-tool-shop-org/si-jam-sessions/pull/10)):
  - anonymous works: first published by 1930 for the US and by 1955 for the EU;
  - CC0 1.0 and the Public Domain Mark.
- **The grand piano** ([#11](https://github.com/mcp-tool-shop-org/si-jam-sessions/pull/11)):
  - the Salamander Grand Piano V3 (Alexander Holm, CC BY 3.0), with 16 velocity layers;
  - `fetch-piano`, which downloads its 742 MB archive and checks it against a pinned SHA-256;
  - `preview`, `notices` and `--voice`.
- **Two arrangements of *Battle Hymn of the Republic***
  ([#14](https://github.com/mcp-tool-shop-org/si-jam-sessions/pull/14)):
  - one by glm-5.3 and one by kimi-k3, both from this project's transcription of the anonymous 1862 Ditson
    edition, dedicated CC0 1.0 and admitted with receipts;
  - a golden of the committed frames for each;
  - `--piece`, with `play` opening on the Battle Hymn;
  - `host notes`, a JSON export of the notes the law commits.
- **The host's exit status** tells a wrong command line (1) from a command that failed while it ran (2). A
  usage error also says that `host help` lists every command.

### Fixed

- Three host findings from external review, all in
  [#11](https://github.com/mcp-tool-shop-org/si-jam-sessions/pull/11):
  - the pre-roll's first fill no longer sets the jam's lookahead;
  - a key held when a MIDI port drops no longer drones;
  - a port-close error at teardown no longer fails a clean jam.

[0.2.0]: https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.2.0
[0.1.0]: https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0
