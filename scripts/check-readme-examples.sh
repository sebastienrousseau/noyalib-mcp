#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Noyalib
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# Verify the README's embedded JSON configuration blocks actually parse.
#
# WHY THIS EXISTS, AND WHY IT IS NOT THE CORE SCRIPT
#
# noyalib's harness compiles every rust block in its README. This repo's
# README has no rust blocks - it documents an editor / agent integration,
# so its examples are JSON configuration a user copies into a settings
# file. Porting the core script here would find nothing, pass trivially,
# and show a green tick for a check that never ran.
#
# What rots in an integration README is the config itself: a renamed key
# or a stray trailing comma, copy-pasted by a user, fails in their editor
# with no obvious link back to these docs.
#
# Blocks tagged with ignore are skipped, the same escape hatch rustdoc
# gives for deliberately partial snippets.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)" || exit 1

README=${1:-README.md}
[ -f "$README" ] || { echo "no $README"; exit 1; }
command -v jq >/dev/null || { echo "  jq not installed - cannot verify"; exit 1; }

tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
fail=0; n=0

awk -v out="$tmp" '
  /^```json/ || /^```jsonc/ {
    if ($0 !~ /ignore/) { i++; f=sprintf("%s/b%03d.json", out, i); inblk=1 }
    next
  }
  /^```/ { inblk=0; f=""; next }
  inblk && f != "" { print >> f }
' "$README"

for f in "$tmp"/*.json; do
  [ -e "$f" ] || continue
  n=$((n + 1))
  if ! jq empty "$f" >/dev/null 2>&1; then
    echo "  [FAIL] a json block in $README is not valid JSON:"
    head -6 "$f" | sed 's/^/        /'
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  echo "-- README JSON blocks do not parse --"
  exit 1
fi
if [ "$n" -eq 0 ]; then
  echo "  [FAIL] no json blocks found - this check would be vacuous"
  exit 1
fi
echo "-- All $n README JSON block(s) parse --"
