# PRD: FEFF10 Full Migration to Rust (Improved)

## 1. Introduction/Overview

Migrate FEFF10 from Fortran to Rust as a full rewrite with final cutover, while preserving exact external behavior for existing user workflows.

This PRD preserves the original intent:
- Improve performance
- Improve reliability/safety
- Improve maintainability

This is a migration project, not a feature-expansion project.

## 2. Goals

- G-1: Preserve externally visible FEFF10 behavior (input contracts, CLI contracts, output artifacts) for all in-scope modules.
- G-2: Reach parity on the approved golden regression suite (100% pass).
- G-3: Improve runtime performance on prioritized workflows.
- G-4: Remove Fortran compiler dependency from the primary build/test/release path.
- G-5: Provide implementation-ready requirements and sequencing for autonomous coding agents.

## 3. Scope

### In Scope

- Full Rust rewrite and cutover of FEFF10 pipelines and modules.
- Exact compatibility for user-visible interfaces.
- Regression harness and golden dataset.
- CI quality gates and parity gates.
- Cutover and rollback readiness.

### Explicit Module Scope (Exhaustive for this PRD)

- Core pipelines: `RDINP`, `POT`, `PATH`, `FMS`, `XSPH`
- Additional scientific modules: `BAND`, `LDOS`, `RIXS`, `CRPA`, `COMPTON`, `DEBYE`, `DMDW`, `SCREEN`, `SELF`, `EELS`, `FULLSPECTRUM`
- Shared foundations: input parsing, numerics/math utilities, CLI, IO, error handling

## 4. Non-Goals (Out of Scope)

- Adding new scientific models or net-new user-facing FEFF capabilities during migration.
- Building a GUI/web UI.
- Bitwise-identical floating-point output across all architectures.
- Maintaining legacy Visual Fortran project files as the primary build system.

## 5. Migration Decisions (Must Be Finalized Before Module Porting)

These are treated as mandatory deliverables, not optional open questions:

- D-1: Platform certification matrix (required OS/arch targets for GA and non-GA).
- D-2: MPI parity scope for v1 cutover (required vs deferred).
- D-3: Numeric comparison policy per output category (exact text vs absolute/relative tolerance thresholds).
- D-4: Compatibility handling for warnings/errors (stderr/stdout text format, error codes, severity mapping).

Decision log reference:
- D-1 status: Approved on 2026-02-16 in `tasks/migration-decision-log.md`.
- D-2 status: Approved on 2026-02-16 as deferred-for-v1 in `tasks/migration-decision-log.md`.
- D-3 status: Approved on 2026-02-16 in `tasks/migration-decision-log.md`; machine-readable policy source is `tasks/numeric-tolerance-policy.json`.
- D-4 status: Approved on 2026-02-16 in `tasks/migration-decision-log.md`; includes exit-code mapping, stderr/stdout diagnostics contract, and legacy failure-class mapping.
- Central contract index (release-blocking): `tasks/migration-contract-reference.md`.
- Compatibility matrix status: Published on 2026-02-16 in `tasks/feff10-compatibility-matrix.md`.
- Fixture manifest status: Published on 2026-02-16 in `tasks/golden-fixture-manifest.json`.
- Fortran-to-Rust boundary map status: Published on 2026-02-16 in `tasks/fortran-rust-boundary-map.md`.

Release-blocking rule for module rewrites:
- All module implementation stories must treat the artifacts indexed in `tasks/migration-contract-reference.md` as mandatory compatibility references.

## 6. Story Execution Order

1. Foundation (must complete first):
   `US-001 -> US-002 -> US-003 -> US-004 -> US-005`
2. Core infrastructure:
   `US-006` and `US-007` can proceed in parallel after foundation.
3. Core scientific path:
   `US-008 -> US-009 -> US-010 -> US-011 -> US-012`
4. Remaining modules (can be parallelized once dependencies are available):
   `US-013` to `US-023`
5. Cross-cutting and release:
   `US-024` (iterative), `US-025` (iterative), `US-026`, `US-027`

## 7. User Stories

### US-001: Lock Migration Decisions
**Description:** As a migration lead, I want mandatory migration decisions finalized so implementation is unblocked and testable.

**Acceptance Criteria:**
- [ ] Platform certification matrix is documented (GA vs non-GA targets).
- [ ] MPI parity decision for v1 cutover is documented (in-scope or explicitly deferred).
- [ ] Numeric tolerance policy is defined per output-file category.
- [ ] Error/warning compatibility strategy is documented.
- [ ] Typecheck passes for any touched tooling or Rust code.

### US-002: Build Compatibility Matrix
**Description:** As a migration engineer, I want an exact compatibility contract so behavior is verifiable.
Primary artifact: `tasks/feff10-compatibility-matrix.md`.

**Acceptance Criteria:**
- [ ] Matrix lists all in-scope modules with required inputs, outputs, and CLI surfaces.
- [ ] Required output filenames and directory structure are documented.
- [ ] Compatibility matrix is traceable to regression fixtures.
- [ ] Typecheck passes for any touched Rust code.

### US-003: Create Golden Fixture Manifest and Baseline Outputs
**Description:** As a migration engineer, I want bounded fixture coverage so parity pass rates are meaningful.

**Acceptance Criteria:**
- [ ] Fixture set includes at least one fixture per in-scope module.
- [ ] Fixture set includes at least one end-to-end multi-module workflow fixture.
- [ ] Fixture set includes at least two documented edge-case fixtures.
- [ ] Baseline outputs are generated from Fortran FEFF10 and stored reproducibly.
- [ ] Each fixture defines comparison mode and pass/fail threshold.
- [ ] Typecheck passes for any touched Rust code.

### US-004: Implement Regression Comparison Harness
**Description:** As a release owner, I want an automated comparator so parity can be enforced continuously.

**Acceptance Criteria:**
- [ ] Comparator supports exact-text and tolerance-based numeric comparisons.
- [ ] Comparator outputs machine-readable and human-readable diff reports.
- [ ] Regression harness runs all fixtures from the manifest in one command.
- [ ] Tests pass for comparator logic.
- [ ] Typecheck passes.

### US-005: Define Rust Workspace Architecture
**Description:** As a developer, I want clear crate boundaries and shared conventions so module ports are safe and consistent.

**Acceptance Criteria:**
- [ ] Workspace layout is defined for parsing, numerics, pipelines, CLI, and shared types.
- [ ] Shared error types and result conventions are implemented.
- [ ] Architecture document maps FEFF10 module boundaries to Rust crates/modules.
- [ ] Architecture planning reflects D-2 by keeping serial-first execution and explicit orchestration boundaries for a future MPI executor.
- [ ] Tests pass for new shared primitives.
- [ ] Typecheck passes.

### US-006: Port FEFF Input Parser
**Description:** As a user, I want existing FEFF input files to parse without changes.

**Acceptance Criteria:**
- [ ] Parser accepts all input constructs used by the fixture manifest.
- [ ] Invalid cards produce deterministic documented errors.
- [ ] Parser output is snapshot-testable.
- [ ] Tests pass for parser fixtures.
- [ ] Typecheck passes.

### US-007: Port Shared Numerics/Math Foundation
**Description:** As a developer, I want validated numerical primitives in Rust so module ports can reuse trusted components.

**Acceptance Criteria:**
- [ ] Core numerical routines required by core pipelines are implemented.
- [ ] Numeric routines are validated against baseline reference values.
- [ ] Precision and formatting behavior follows D-3 policy.
- [ ] Tests pass for math/numerics crates.
- [ ] Typecheck passes.

### US-008: Port RDINP
**Description:** As a user, I want Rust `RDINP` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `RDINP` fixture outputs match baseline under approved comparison rules.
- [ ] `RDINP` default handling matches documented compatibility contract.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-009: Port POT
**Description:** As a user, I want Rust `POT` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `POT` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-010: Port PATH
**Description:** As a user, I want Rust `PATH` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `PATH` fixture outputs match baseline under approved comparison rules.
- [ ] Path ordering/filtering behavior matches compatibility contract.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-011: Port FMS
**Description:** As a user, I want Rust `FMS` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `FMS` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-012: Port XSPH
**Description:** As a user, I want Rust `XSPH` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `XSPH` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-013: Port BAND
**Description:** As a user, I want Rust `BAND` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `BAND` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-014: Port LDOS
**Description:** As a user, I want Rust `LDOS` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `LDOS` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-015: Port RIXS
**Description:** As a user, I want Rust `RIXS` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `RIXS` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-016: Port CRPA
**Description:** As a user, I want Rust `CRPA` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `CRPA` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-017: Port COMPTON
**Description:** As a user, I want Rust `COMPTON` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `COMPTON` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-018: Port DEBYE
**Description:** As a user, I want Rust `DEBYE` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `DEBYE` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-019: Port DMDW
**Description:** As a user, I want Rust `DMDW` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `DMDW` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-020: Port SCREEN
**Description:** As a user, I want Rust `SCREEN` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `SCREEN` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-021: Port SELF
**Description:** As a user, I want Rust `SELF` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `SELF` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-022: Port EELS
**Description:** As a user, I want Rust `EELS` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `EELS` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-023: Port FULLSPECTRUM
**Description:** As a user, I want Rust `FULLSPECTRUM` behavior equivalent to FEFF10.

**Acceptance Criteria:**
- [ ] `FULLSPECTRUM` fixture outputs match baseline under approved comparison rules.
- [ ] Module-specific regression tests pass.
- [ ] Typecheck passes.

### US-024: Implement CLI and File-Contract Compatibility
**Description:** As an existing FEFF user, I want existing automation to keep working without script rewrites.

**Acceptance Criteria:**
- [ ] Required CLI commands/options from the compatibility matrix are implemented.
- [ ] Required output filenames and directories are preserved.
- [ ] Exit code behavior follows compatibility contract.
- [ ] Integration tests pass for CLI compatibility.
- [ ] Typecheck passes.

### US-025: Add CI Quality and Parity Gates
**Description:** As a release owner, I want CI to block incompatible builds.

**Acceptance Criteria:**
- [ ] CI runs `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, and `cargo fmt --check`.
- [ ] CI runs fixture regression harness and fails on parity regressions.
- [ ] CI planning reflects D-2 by making serial parity release-blocking and treating MPI validation as non-blocking until MPI scope is re-opened.
- [ ] CI publishes regression diff artifacts on failure.
- [ ] CI pipeline tests pass.
- [ ] Typecheck passes.

### US-026: Publish Migration and Operator Documentation
**Description:** As a maintainer, I want clear docs so users and developers can run, validate, and troubleshoot Rust FEFF10.

**Acceptance Criteria:**
- [ ] Developer docs describe build, test, and regression workflows.
- [ ] User docs describe runtime usage, compatibility guarantees, and known limits.
- [ ] Troubleshooting docs map common failures to actions.
- [ ] Documentation checks/tests pass (if docs tooling exists).
- [ ] Typecheck passes for any touched Rust tooling code.

### US-027: Execute Cutover and Rollback Validation
**Description:** As a release owner, I want a rehearsed cutover and rollback procedure so production risk is controlled.

**Acceptance Criteria:**
- [ ] Cutover checklist is documented and executed in a rehearsal environment.
- [ ] Rollback is explicitly defined (artifact reversion and operational steps) and tested in rehearsal.
- [ ] Release artifacts are published from Rust-native build path.
- [ ] Final end-to-end regression pass is green before GA cutover.
- [ ] Typecheck passes.

## 8. Functional Requirements

- FR-1: The system must provide an exhaustive compatibility matrix for all modules in Section 3.
- FR-2: The system must finalize platform certification targets before module porting starts.
- FR-3: The system must explicitly decide MPI parity scope for v1 before parallel-sensitive modules are implemented.
- FR-4: The system must define numeric comparison policy per output category before parity claims are accepted.
- FR-5: The system must parse existing FEFF inputs used by approved fixtures without requiring format changes.
- FR-6: The system must preserve required CLI command/option contracts.
- FR-7: The system must preserve required output file names and directory structure.
- FR-8: The system must define and preserve compatible exit-code semantics.
- FR-9: The system must define and preserve compatible warning/error behavior.
- FR-10: The system must include a regression harness supporting exact and tolerance-based comparisons.
- FR-11: The system must include at least one fixture per in-scope module.
- FR-12: The system must include at least one end-to-end multi-module fixture.
- FR-13: The system must include at least two documented edge-case fixtures.
- FR-14: The system must implement Rust equivalents for `RDINP`, `POT`, `PATH`, `FMS`, and `XSPH` with passing regressions.
- FR-15: The system must implement Rust equivalents for `BAND`, `LDOS`, `RIXS`, `CRPA`, `COMPTON`, `DEBYE`, `DMDW`, `SCREEN`, `SELF`, `EELS`, and `FULLSPECTRUM` with passing regressions.
- FR-16: The system must run CI gates for typecheck, tests, lint, and formatting.
- FR-17: The system must fail CI when parity regressions are detected.
- FR-18: The system must publish migration and operational documentation before cutover.
- FR-19: The system must define and rehearse rollback before GA cutover.
- FR-20: The primary supported build/test path after cutover must not require a Fortran compiler.
- FR-21: Any intentional compatibility deviation must be documented and approved before release.

## 9. Design Considerations

- Keep crate/module boundaries aligned with FEFF10 domain boundaries for easier validation.
- Prefer explicit typed data flow over global mutable state.
- Keep output serialization deterministic where feasible to reduce noisy diffs.

## 10. Technical Considerations

- Decide Rust numerical library strategy for FEFF10-equivalent math operations early.
- Define deterministic output formatting rules with D-3 tolerance policy.
- D-2 is currently deferred: keep serial execution as the only GA runtime path for v1.
- Preserve execution-orchestration seams so MPI runtime integration can be added later without changing module-level scientific contracts.
- Keep fixture runtime manageable by splitting CI into smoke and full parity stages if needed.

## 11. Success Metrics

- SM-1: 100% pass rate on approved golden regression fixtures.
- SM-2: 100% pass rate on required Rust CI quality gates.
- SM-3: Performance on reference platform:
  - At least 10% faster median wall-clock time on the top 5 prioritized production workflows.
  - No approved workflow is slower than 1.10x the Fortran baseline.
- SM-4: 0 critical compatibility regressions during cutover rehearsal.
- SM-5: Fortran compiler is not required in primary GA release pipeline.

## 12. Open Questions (Non-Blocking)

- OQ-1: What timeline and staffing model is approved for this migration plan?
- OQ-2: Which modules, if any, should receive additional optimization after parity cutover?
