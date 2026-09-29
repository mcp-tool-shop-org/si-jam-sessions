---
title: The container
description: Run the host from the published image, without installing Rust.
sidebar:
  order: 7
---

The image `ghcr.io/mcp-tool-shop-org/si-jam-sessions` carries the `host` binary and the scores in this
repository. It does not carry the piano samples. Those stay a download, checked against the same SHA-256 the
source pins.

The image is published when a version tag is pushed. `0.2.1` is
`ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1`, and that tag also moves `:0.2` and `:latest`.

## What it can do

`render`, `notes`, `notices`, `preview` and `help` need no sound device. `play` and `jam` open an audio
device. Playing through a Linux device is untested, in a container and outside one, and live MIDI input is
Windows-only. Use the container to render and to export notes. Use a Windows build to play along.

The process starts as root only long enough to drop privileges. It then runs as uid 10001, unless you set
`HOST_UID` and `HOST_GID` to your own, which is what you want when the command writes into a directory you
mounted.

The working directory inside the image is the one that holds `scores/`. Leave it there. The host finds a
score by walking up from that directory. Mount a folder for the files you want back, and pass a path inside
that folder.

## Render the score on the oscillator

```bash
docker pull ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1

docker run --rm \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -v "$PWD:/out" \
  ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1 \
  render /out/battle-hymn.wav --piece battle-hymn-glm-5.3 --voice osc
```

On Windows PowerShell, the same command is:

```powershell
docker run --rm `
  -e HOST_UID=10001 `
  -v "${PWD}:/out" `
  ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1 `
  render /out/battle-hymn.wav --piece battle-hymn-glm-5.3 --voice osc
```

Without `HOST_UID`, the file is owned by uid 10001.

## Notes the law commits

```bash
docker run --rm \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -v "$PWD:/out" \
  ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1 \
  notes /out/notes.json --piece battle-hymn-kimi-k3
```

## The piano

`fetch-piano` uses `curl`, which the image includes. Give it a directory you can keep, and pass that
directory as `--samples` when you render:

```bash
docker run --rm \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -v "$PWD/piano:/piano" \
  ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1 \
  fetch-piano --dir /piano

docker run --rm \
  -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
  -v "$PWD:/out" -v "$PWD/piano:/piano" \
  ghcr.io/mcp-tool-shop-org/si-jam-sessions:0.2.1 \
  render /out/battle-hymn.wav --piece battle-hymn-glm-5.3 --voice piano --samples /piano
```

The archive is 742 MB. The image checks it before unpacking, and it refuses a path that climbs out of the
directory you named.

## Build the image yourself

From a clone, at the repository root:

```bash
docker build -t si-jam-sessions:local .
docker run --rm si-jam-sessions:local help
```

The build uses the toolchain in `rust-toolchain.toml` and `cargo build -p host --release --locked`.
