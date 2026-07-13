# syntax=docker/dockerfile:1.7
# SPDX-FileCopyrightText: 2026 Noyalib
# SPDX-License-Identifier: MIT OR Apache-2.0

# pkg/docker/Dockerfile.mcp — Model Context Protocol server image.
#
# The MCP server speaks JSON-RPC over stdio so AI agents (Claude
# Code, GitHub Copilot, etc.) can call YAML-manipulation tools
# (`parse`, `format`, `get`, `set`, `validate`) without the agent
# needing a Rust toolchain.
#
# Distribution channels:
#   - GHCR (this image): ghcr.io/sebastienrousseau/noyalib-mcp:<tag>
#   - npx wrapper:       npx noyalib-mcp  (downloads + caches the
#                                          platform-appropriate
#                                          binary from a GitHub
#                                          Release on first run)

FROM rust:1.96-bookworm@sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663 AS build

WORKDIR /src
COPY . .

ARG SOURCE_DATE_EPOCH
ENV SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-0}
ENV RUSTFLAGS="--remap-path-prefix=/src=/build --remap-path-prefix=/usr/local/cargo=/cargo"

RUN cargo build --release --locked

# ── Runtime stage ────────────────────────────────────────────────
FROM gcr.io/distroless/cc-debian12:nonroot@sha256:ce0d66bc0f64aae46e6a03add867b07f42cc7b8799c949c2e898057b7f75a151

LABEL org.opencontainers.image.title="noyalib-mcp"
LABEL org.opencontainers.image.description="MCP server for noyalib YAML tools — parse, format, get, set, validate over JSON-RPC stdio."
LABEL org.opencontainers.image.source="https://github.com/sebastienrousseau/noyalib-mcp"
LABEL org.opencontainers.image.licenses="MIT OR Apache-2.0"

COPY --from=build /src/target/release/noyalib-mcp /usr/local/bin/noyalib-mcp

USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/noyalib-mcp"]
