<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# Published MCP 0.0.58 source review

Reviewed on 2026-11-10. Authenticated registry archive SHA-256:
`ef34eef8f605fb24f4f1fb5c86dd138e03fc7c3ca055cfa5a498732bcbf53c3a`.
All 17 packaged Rust/build files match the reviewed current source.
Refreshing cargo-vet's unpublished-version map correctly requires this
actual published baseline for the local 0.0.59 package. The previous map
was stale; an exemption or a certificate for an unpublished package would
not supply this evidence.

The full runtime source was read: library/tool dispatch, filesystem
operations and Unix path walking, limits, prompts/resources, transports,
HTTP guards, session wrapper and SSE bridge. The build script only
configures an opt-in coverage cfg and reads its declared environment flag.
The crate forbids unsafe code.

Unix file access walks from an open root handle with no-follow directory
opens, bounded symlink resolution and no lexical escape. Reads require
regular files, cap metadata length and stop one byte beyond the input
limit. Replacements use exclusive random temporary names, preserve
permissions, sync before rename and clean up failures. Direct library file
functions intentionally act on caller-supplied paths; protocol tools use
the root capability. Non-Unix canonicalization and the public confine
helper are not a protection against a hostile local filesystem actor.

YAML text and fragments are bounded before parsing. CST edits use the
parser configuration and propagate failures. Schema text, diagnostics and
violation counts have explicit limits. Prompts escape control characters
and bound user names; resources return static catalogue/schema data.
Blocking tool work runs outside async executor tasks; a deadline abandons
its result but cannot interrupt work already running. Parser budgets and
external deployment limits remain necessary.

HTTP transports require explicit selection, default to loopback and
apply Host/Origin checks. SSE posts require JSON, bounded queues are used,
and session drops cancel work/remove state. Binding non-loopback does not
add authentication: a trusted gateway or equivalent access control is a
deployment responsibility. This review does not prove hard global resource
bounds under every concurrent schedule or platform filesystem race.

The current source passed 124 tests, including real protocol and transport
cases, under the independently reviewed dependency graph. Dependency
source certificates remain separate; this audit certifies the published
crate's own source and build script, not its historical dependency closure.
