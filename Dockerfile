# syntax=docker/dockerfile:1

# The host binary and the committed scores. The piano samples are not in the
# image: `fetch-piano` downloads them and checks the pinned SHA-256.
#
# The host finds a score by walking up from the working directory until it
# sees scores/entertainer/receipt.json, so the working directory stays
# /opt/si-jam-sessions. Write output to a mounted path, not by changing
# the working directory.
#
# The toolchain is whatever rust-toolchain.toml names.

FROM debian:bookworm-slim AS build

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        build-essential \
        pkg-config \
        libasound2-dev \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain none --profile minimal

WORKDIR /src
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN rustup show active-toolchain \
    && cargo build -p host --release --locked \
    && strip target/release/host

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        libasound2 \
        passwd \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 --shell /usr/sbin/nologin jam

COPY --from=build /src/target/release/host /usr/local/bin/host
COPY docker/entrypoint.sh /usr/local/bin/si-jam-entrypoint
RUN chmod 0755 /usr/local/bin/si-jam-entrypoint /usr/local/bin/host
COPY scores /opt/si-jam-sessions/scores

WORKDIR /opt/si-jam-sessions

ARG VERSION=0.2.0
ARG REVISION=unknown
LABEL org.opencontainers.image.title="si-jam-sessions" \
      org.opencontainers.image.description="Deterministic music host: render and grade a score without installing Rust." \
      org.opencontainers.image.source="https://github.com/mcp-tool-shop-org/si-jam-sessions" \
      org.opencontainers.image.url="https://mcp-tool-shop-org.github.io/si-jam-sessions/" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}" \
      org.opencontainers.image.vendor="MCP Tool Shop"

ENTRYPOINT ["si-jam-entrypoint"]
CMD ["help"]
