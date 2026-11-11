<!-- SPDX-FileCopyrightText: 2026 Noyalib -->
<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# 0002. Bound blocking work by job lifetime

- Status: proposed; implemented on the future branch, independent review pending
- Date: 2026-11-11

## Context

Parsing and file operations run on Tokio's blocking pool. A request timeout
or client cancellation does not interrupt a running blocking closure.
Repeated ended requests could therefore accumulate still-running work.
The HTTP session cap controls a different resource; stateless requests and
multiple handler instances also reach the blocking pool.

## Decision

Use one process-wide semaphore with 64 permits for the MCP tool router.
The initial ceiling matches the default HTTP session count, allowing one
outstanding operation per default session while bounding all transports
and handler instances together. This is an admission limit, not a claim
that arbitrary input fits a measured RSS budget. Parser and file limits
still apply independently. Changing the ceiling needs updated evidence,
documentation and review.

Acquire a permit before scheduling a blocking job. Refuse excess work with
a readable busy tool failure; do not append it to an unbounded work queue.
Move the owned permit into the blocking closure. The request's timeout,
future or client connection does not own it. Completion, ordinary failure
or panic releases the permit when the closure ends.

A blocked operating-system operation can retain capacity after its request
ends. This preserves the admission bound; it does not make cancellation a
transaction rollback. Callers must verify uncertain mutations before retrying.
The public Rust types, methods, Cargo features, dependencies and Rust floor
remain unchanged. Synchronous embedding functions do not spawn this work;
the application router owns the policy. Core alloc-only behavior is separate.

## Alternatives

A permit held by the request future would be returned on cancellation or
timeout while its blocking work continued. A per-handler budget would be
multiplied by independent sessions or stateless handler construction.
Waiting without a separate bound would accumulate queued request state.
None establishes the intended process-wide job bound.

## Evidence

The existing timeout test detects a deliberately ignored timeout before
executor extraction. The unchanged extraction passes the full 132-test
battery. With admission unenforced, both controlled timeout and cancellation
cases start a second job while the first remains blocked. They fail before
the guard and pass after it. A channel observation checks that refused work
never executes, without relying on a sleep. Further cases prove capacity
returns after normal completion and panic. The full corrected battery
passes 135 tests; strict clippy and the dependency policies pass.

These are local observations. Hosted platform checks, independent review
and registry-source release gates retain their own acceptance records.
