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

FROM rust:1.96-bookworm@sha256:19817ead3289c8c631c73df281e18b59b172f6a31f4f563290f69cddd06c30e9 AS build

WORKDIR /src
COPY . .

ARG SOURCE_DATE_EPOCH
ENV SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH:-0}
ENV RUSTFLAGS="--remap-path-prefix=/src=/build --remap-path-prefix=/usr/local/cargo=/cargo"

RUN cargo build --release --locked

# ── Runtime stage ────────────────────────────────────────────────
FROM gcr.io/distroless/cc-debian12:nonroot@sha256:b0ae8e989418b458e0f25489bc3be523718938a2b70864cc0f6a00af1ddbd985

LABEL org.opencontainers.image.title="noyalib-mcp"
LABEL org.opencontainers.image.description="MCP server for noyalib YAML tools — parse, format, get, set, validate over JSON-RPC stdio."
LABEL org.opencontainers.image.source="https://github.com/sebastienrousseau/noyalib-mcp"
LABEL org.opencontainers.image.licenses="MIT OR Apache-2.0"

COPY --from=build /src/target/release/noyalib-mcp /usr/local/bin/noyalib-mcp

USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/noyalib-mcp"]
