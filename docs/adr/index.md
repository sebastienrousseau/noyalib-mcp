<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Architecture decision records

One file per decision that shaped this server: context, options,
decision, consequences. Change a decision by adding a new record that
supersedes the old one. Decisions that shape the whole family — the
lockstep release model, the workspace split — live in the core
repository's [`docs/adr/`](https://github.com/sebastienrousseau/noyalib/tree/main/docs/adr).

- [0001. Serve stdio, streamable HTTP and SSE from one command line](0001-three-transports-one-command-line.md)
