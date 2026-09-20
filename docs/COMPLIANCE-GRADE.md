<!-- SPDX-License-Identifier: Apache-2.0 OR MIT -->

# Repository standard compliance grade

Assessment date: 2026-09-20. Source rubric:
`/Users/seb/Code/REPO-STANDARD.md`. This is a conservative static assessment of
checked-in evidence, not a substitute for successful CI runs.

## Result

**15/24 signals (62%). README template: pass. Strict candidate tier: L1.**

| Category | Signals | Summary |
| :--- | :---: | :--- |
| Identity and README | 3/3 | Canonical structure and MCP-specific policy present |
| Documentation | 2/3 | Manual, architecture, and ADR evidence present |
| Build and install UX | 1/3 | Native build present; generated completions/manpage signals are absent |
| Releases and binaries | 2/3 | Automated releases present; complete L3 evidence remains open |
| Packaging | 2/3 | Container distribution present; Repology/reproducibility are incomplete |
| CI quality gates | 2/3 | Matrix and coverage signals present |
| Supply chain | 2/3 | Foundation controls present; advisory-audit detection remains open |
| Community | 1/3 | Foundation governance present; L2 template and docs-lint signals are open |

The cumulative tier remains L1. Priority work is generated CLI assets,
distribution tracking and reproducibility, explicit advisory-audit CI, and
audit-visible issue/PR templates and markdown lint enforcement.
