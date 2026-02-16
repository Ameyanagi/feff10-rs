# FEFF10 Rust Migration Decision Log

This log records finalized migration governance decisions that unblock implementation.

## D-1: Platform Certification Matrix

### Certification tiers
- `GA`: release-blocking targets. CI, packaging, and smoke validation must pass.
- `Non-GA`: best-effort targets. Failures are tracked but do not block release.

### Approved platform matrix

| Tier | OS baseline | Architecture | Rust target triple | Compiler constraints | Runtime constraints | Packaging/Artifact scope |
| --- | --- | --- | --- | --- | --- | --- |
| GA | Ubuntu 22.04 LTS | x86_64 | `x86_64-unknown-linux-gnu` | Stable Rust toolchain only (`rustup default stable`), edition `2024`, no nightly-only features | `glibc >= 2.35`, POSIX shell scripts require `bash` and coreutils | Primary release binaries and release archives |
| GA | macOS 14 Sonoma | arm64 (Apple Silicon) | `aarch64-apple-darwin` | Stable Rust toolchain only, edition `2024`, build with Apple Command Line Tools installed | Native Apple runtime; unsigned local binaries allowed for dev/test, signed artifacts required at release time | Release binaries and smoke-test notarization candidate |
| Non-GA | Ubuntu 22.04 LTS | arm64 | `aarch64-unknown-linux-gnu` | Stable Rust toolchain only; cross-build allowed from GA Linux x86_64 builders | `glibc >= 2.35`; parity harness may be reduced to smoke set if runtime is constrained | Preview binaries only (no GA SLA) |
| Non-GA | Windows 11 | x86_64 | `x86_64-pc-windows-msvc` | Stable Rust with MSVC toolchain; no GNU Windows target in v1 | Microsoft Visual C++ Redistributable runtime required for distributed binaries | Preview CLI artifacts; no release-blocking guarantee |
| Non-GA | macOS 13 Ventura | x86_64 (Intel) | `x86_64-apple-darwin` | Stable Rust toolchain only; Rosetta is not a supported execution dependency | Native Intel execution only; no arm64 translation dependency assumptions | Preview binaries only (best effort) |

### CI and release implications
- Release-blocking CI matrix must include both GA rows.
- Non-GA targets can run in scheduled or manual workflows, not required in PR-blocking jobs.
- Packaging and smoke-test automation must treat GA targets as mandatory and fail-fast.

## D-2: MPI Parity Scope for v1 Cutover

### Decision
- MPI execution parity is deferred for the Rust v1 cutover.
- Rust v1 scope is single-process parity for approved fixtures and compatibility contracts.

### Deferred-scope fallback behavior
- Distributed or `mpirun`-based production workflows remain on the existing Fortran FEFF10 MPI binaries until MPI parity is delivered.
- Rust v1 user and operator documentation must mark MPI execution as explicitly unsupported for cutover GA.
- Regression and release readiness for v1 are evaluated on serial workflows only.

### Roadmap note
- Revisit MPI parity after serial cutover stability is demonstrated and core module parity stories are green.
- A future MPI story must define runtime and dependency choices, plus portability and diagnostics contracts, before enabling release-blocking MPI CI.

### Architecture planning implications
- Core Rust pipeline APIs should keep execution orchestration boundaries explicit so an MPI-capable executor can be introduced later without changing scientific module contracts.
- Migration sequencing should prioritize module parity in serial mode before introducing distributed execution semantics.

### CI planning implications
- PR-blocking CI for v1 excludes MPI runtime setup and validates only serial parity and quality gates.
- MPI validation can run as non-blocking exploratory jobs once MPI implementation stories begin, and becomes release-blocking only after a separate approval.

## D-3: Numeric Tolerance Policy for Regression Parity

### Decision
- Regression comparison must use category-based policies defined in `tasks/numeric-tolerance-policy.json`.
- Comparator default mode is `exact_text` for any output artifact that does not match an explicit category rule.
- Numeric comparison passes when either threshold is satisfied:
  - `abs(actual - baseline) <= absTol`
  - `abs(actual - baseline) <= relTol * max(abs(baseline), relativeFloor)`

### Output category policy matrix

| Category ID | Mode | Output patterns | Absolute tolerance | Relative tolerance |
| --- | --- | --- | --- | --- |
| `diagnostic_logs` | `exact_text` | `**/*.log`, `**/*.err`, `**/warnings*.txt` | N/A | N/A |
| `columnar_spectra` | `numeric_tolerance` | `**/xmu.dat`, `**/chi.dat`, `**/danes.dat`, `**/eels.dat`, `**/compton.dat`, `**/rixs*.dat` | `1e-8` | `1e-6` |
| `density_tables` | `numeric_tolerance` | `**/ldos*.dat`, `**/rho*.dat` | `5e-8` | `5e-6` |
| `path_scattering_tables` | `numeric_tolerance` | `**/feff*.dat`, `**/paths.dat`, `**/path*.dat` | `1e-7` | `1e-5` |
| `thermal_workflow_tables` | `numeric_tolerance` | `**/*.dmdw.out`, `**/debye*.dat`, `**/sig*.dat` | `1e-6` | `1e-4` |
| `structured_reports` | `exact_text` | `**/*.json`, `**/*.toml` | N/A | N/A |

### Comparator ingestion contract
- Comparator must load `tasks/numeric-tolerance-policy.json` before evaluating fixtures.
- Category resolution is first-match in file order; unmatched files use `defaultMode`.
- Numeric tokenization must support Fortran exponent notation (`D`/`d` as `E`).
- Mismatched line counts, token counts, or NaN/Inf mismatches are hard failures.

## Approval Record: D-1

- Decision ID: `D-1`
- Decision title: Finalize platform certification matrix
- Status: `Approved`
- Approved on: `2026-02-16`
- Approved by: FEFF10 Rust migration lead
- Scope: Applies to v1 migration cutover planning and all downstream CI/release stories
- Review trigger: Re-open only if a GA target becomes unsupported by Rust stable toolchain or release infra

## Approval Record: D-2

- Decision ID: `D-2`
- Decision title: Finalize MPI parity scope
- Status: `Approved`
- Approved on: `2026-02-16`
- Approved by: FEFF10 Rust migration lead
- Scope: Applies to v1 architecture sequencing, CI planning, and release readiness criteria
- Review trigger: Re-open when MPI parity implementation is prioritized for GA scope

## Approval Record: D-3

- Decision ID: `D-3`
- Decision title: Define numeric tolerance policy
- Status: `Approved`
- Approved on: `2026-02-16`
- Approved by: FEFF10 Rust migration lead
- Scope: Applies to regression policy, fixture thresholds, and comparator implementation behavior
- Review trigger: Re-open when a fixture category requires a tolerance change outside approved ranges
