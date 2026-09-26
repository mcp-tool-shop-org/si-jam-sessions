---
title: Getting started
description: Build si-jam-sessions, fetch the grand piano, and play, render or jam.
sidebar:
  order: 1
---

## What you need

- **Rust.** The repository pins version 1.98.1 in `rust-toolchain.toml`, with the wasm target, rustfmt and
  clippy. [rustup](https://rustup.rs) installs all of it on the first build.
- **git**, to clone the repository.
- **About 1.5 GB of free disk** while the piano is fetched: the 742 MB archive and the 714 MB of samples
  unpacked from it. The archive is deleted afterwards unless you pass `--keep-archive`.

### Platforms

| Platform | What works |
|---|---|
| Windows 10 and 11 | Everything: playing, rendering, and live input from a MIDI keyboard or the computer keyboard. Live input has not yet been tried with a real MIDI keyboard |
| Linux | CI builds and tests the host and renders with it. Playing through a Linux audio device is untested, and live input is Windows-only for now |
| macOS | Untested |

## Build and play

```bash
git clone https://github.com/mcp-tool-shop-org/si-jam-sessions
cd si-jam-sessions
cargo run -p host --release -- devices
```

`devices` lists the audio outputs and MIDI inputs, each with an index.

### Fetch the grand piano once

```bash
cargo run -p host --release -- fetch-piano
```

This downloads the Salamander Grand Piano V3 (Alexander Holm, CC BY 3.0) from FreePats, through your system's
`curl`. It checks the archive's size and SHA-256 against the values pinned in the source before it unpacks
anything. The samples go into a per-user cache, never into the repository. A download that stopped resumes the
next time you run it.

Without the piano, the host plays the score on a plain oscillator.

### Listen

```bash
cargo run -p host --release -- play                                # glm-5.3's arrangement
cargo run -p host --release -- play --piece battle-hymn-kimi-k3    # kimi-k3's
cargo run -p host --release -- play --piece entertainer            # The Entertainer, with a take
```

`play` opens on the Battle Hymn as glm-5.3 arranged it. `--output 2` or `--output speakers` picks an output
from `devices`.

### Render to a file

```bash
cargo run -p host --release --locked -- render battle-hymn-glm-5.3.wav --piece battle-hymn-glm-5.3 --voice piano
```

The WAV is 48 kHz, stereo, 32-bit float. It carries the piano's credit in its `LIST/INFO` chunk. Rendering is
deterministic: a second render gives the same bytes. The release's hashes were measured on Windows, and CI's
piano job on Linux printed the same two. Check a render against the [release](https://github.com/mcp-tool-shop-org/si-jam-sessions/releases/tag/v0.1.0).

### Play along

```bash
cargo run -p host --release -- jam --midi 0      # a MIDI keyboard, by index or name
cargo run -p host --release -- jam --keyboard    # the computer keyboard
```

`jam` plays the score and a click, takes your live take, and prints a verdict for every note as it lands.

:::caution[Use a wired output]
A Bluetooth output's delay is longer than the 100 ms the law allows for delivering a note, so notes played
through one arrive too late to grade fairly.
:::

## Check the build

The repository's own gates are the ones CI runs:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run --locked -p golden --bin write-golden -- --check
```

The last command regenerates every golden and compares it with the committed files. It exits 1 and names the
line if anything differs.
