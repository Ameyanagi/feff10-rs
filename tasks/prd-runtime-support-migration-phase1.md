# PRD: Runtime-Support Dependency Migration (Phase 1)

## 1. Introduction/Overview

This PRD defines Phase 1 of "full migration" after the 16-module v1 runtime closeout.

Current state from `gap/fortran-to-rust-file-gap.csv`:
- `runtime_owned`: 272 files
- `runtime_support_dependency`: 153 files
- `out_of_scope`: 102 files

Phase 1 scope is to migrate `runtime_support_dependency` to Rust while deferring `out_of_scope`.
Delivery is module-by-module end-to-end, with strict requirement that existing 16-module quality and oracle parity gates remain green in every batch.
This phase also includes required toolchain/docs updates to keep migration status auditable.

## 2. Goals

- Reduce `runtime_support_dependency` from `153` to `0`.
- Keep existing 16-module oracle parity green (`17/17` fixtures passing).
- Keep strict quality gates green:
  - `cargo check --locked`
  - `cargo test --locked`
  - `cargo clippy --locked --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
- Preserve current artifact contracts and tolerance policy strictness (no global relaxation).
- Publish updated migration documentation reflecting Phase 1 completion and remaining deferred scope.

## 3. User Stories

### US-001: Add module-scoped runtime-support tracking
**Description:** As a maintainer, I want module-level gap visibility so each migration batch can prove end-to-end closure per module.

**Acceptance Criteria:**
- [ ] Extend `scripts/fortran/generate-gap-report.py` to emit module-scoped support counts (per runtime module).
- [ ] Add report output section with per-module `runtime_support_dependency` remaining count.
- [ ] Regenerated `gap/` artifacts are deterministic across repeated runs.
- [ ] Typecheck passes.

### US-002: Close POT runtime-support gap
**Description:** As a maintainer, I want POT to have no remaining runtime-support dependency files so POT runtime is self-contained in Rust scope.

**Acceptance Criteria:**
- [ ] POT module-scoped support count reaches zero in generated gap artifacts.
- [ ] POT parity regression remains green (`crates/feff-core/tests/pot_parity.rs`).
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-003: Close DEBYE runtime-support gap
**Description:** As a maintainer, I want DEBYE to have no remaining runtime-support dependency files so DEBYE runtime is self-contained in Rust scope.

**Acceptance Criteria:**
- [ ] DEBYE module-scoped support count reaches zero in generated gap artifacts.
- [ ] DEBYE parity regression remains green (`crates/feff-core/tests/debye_parity.rs`).
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-004: Close PATH runtime-support gap
**Description:** As a maintainer, I want PATH to have no remaining runtime-support dependency files so PATH runtime is self-contained in Rust scope.

**Acceptance Criteria:**
- [ ] PATH module-scoped support count reaches zero in generated gap artifacts.
- [ ] PATH parity regression remains green (`crates/feff-core/tests/path_parity.rs`).
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-005: Close FMS runtime-support gap
**Description:** As a maintainer, I want FMS to have no remaining runtime-support dependency files so FMS runtime is self-contained in Rust scope.

**Acceptance Criteria:**
- [ ] FMS module-scoped support count reaches zero in generated gap artifacts.
- [ ] FMS parity regression remains green (`crates/feff-core/tests/fms_parity.rs`).
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-006: Close LDOS runtime-support gap
**Description:** As a maintainer, I want LDOS to have no remaining runtime-support dependency files so LDOS runtime is self-contained in Rust scope.

**Acceptance Criteria:**
- [ ] LDOS module-scoped support count reaches zero in generated gap artifacts.
- [ ] LDOS parity regression remains green (`crates/feff-core/tests/ldos_parity.rs`).
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-007: Close SELF runtime-support gap
**Description:** As a maintainer, I want SELF to have no remaining runtime-support dependency files so SELF runtime is self-contained in Rust scope.

**Acceptance Criteria:**
- [ ] SELF module-scoped support count reaches zero in generated gap artifacts.
- [ ] SELF parity regression remains green (`crates/feff-core/tests/self_parity.rs`).
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-008: Close RIXS runtime-support gap
**Description:** As a maintainer, I want RIXS to have no remaining runtime-support dependency files so RIXS runtime is self-contained in Rust scope.

**Acceptance Criteria:**
- [ ] RIXS module-scoped support count reaches zero in generated gap artifacts.
- [ ] RIXS parity regression remains green (`crates/feff-core/tests/rixs_parity.rs`).
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-009: Close EELS runtime-support gap
**Description:** As a maintainer, I want EELS to have no remaining runtime-support dependency files so EELS runtime is self-contained in Rust scope.

**Acceptance Criteria:**
- [ ] EELS module-scoped support count reaches zero in generated gap artifacts.
- [ ] EELS parity regression remains green (`crates/feff-core/tests/eels_parity.rs`).
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-010: Close FULLSPECTRUM runtime-support gap
**Description:** As a maintainer, I want FULLSPECTRUM to have no remaining runtime-support dependency files so FULLSPECTRUM runtime is self-contained in Rust scope.

**Acceptance Criteria:**
- [ ] FULLSPECTRUM module-scoped support count reaches zero in generated gap artifacts.
- [ ] FULLSPECTRUM parity regression remains green (`crates/feff-core/tests/fullspectrum_parity.rs`).
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-011: Close COMPTON runtime-support gap
**Description:** As a maintainer, I want COMPTON to have no remaining runtime-support dependency files so COMPTON runtime is self-contained in Rust scope.

**Acceptance Criteria:**
- [ ] COMPTON module-scoped support count reaches zero in generated gap artifacts.
- [ ] COMPTON parity regression remains green (`crates/feff-core/tests/compton_parity.rs`).
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-012: Close XSPH runtime-support gap
**Description:** As a maintainer, I want XSPH to have no remaining runtime-support dependency files so XSPH runtime is self-contained in Rust scope.

**Acceptance Criteria:**
- [ ] XSPH module-scoped support count reaches zero in generated gap artifacts.
- [ ] XSPH parity regression remains green (`crates/feff-core/tests/xsph_parity.rs`).
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-013: Burn down shared multi-module support remainder
**Description:** As a maintainer, I want shared support code used across multiple runtime modules to be migrated so no runtime-support dependencies remain.

**Acceptance Criteria:**
- [ ] Shared support remainder across `COMMON`, `KSPACE`, `MATH`, `TDLDA`, `FOVRG`, `EXCH`, `IOMODS`, `ERRORMODS`, `INPGEN`, `PAR`, `UTILITY`, `RHORRP` is migrated for runtime usage paths.
- [ ] Global `runtime_support_dependency` count reaches `0` in `gap/fortran-to-rust-file-gap.csv`.
- [ ] Existing oracle gate still reports zero failed/mismatched fixtures.
- [ ] Typecheck passes.

### US-014: Complete phase-1 docs and toolchain cleanup
**Description:** As a maintainer, I want migration docs and automation to reflect phase-1 closure so status is auditable and reproducible.

**Acceptance Criteria:**
- [ ] Update `tasks/migration-gap-analysis.md` with phase-1 results and deferred phase-2 scope.
- [ ] Update `README.md` and `docs/developer-workflows.md` references to include new gap-report workflow.
- [ ] Add one reproducible command block for regenerating gap artifacts from clean checkout.
- [ ] Typecheck passes.

## 4. Functional Requirements

- FR-1: The system must provide module-scoped runtime-support counts in generated gap artifacts.
- FR-2: For each in-scope module story, module-scoped `runtime_support_dependency` must reach zero before story completion.
- FR-3: The system must keep existing 16-module oracle parity gate green after each story/batch.
- FR-4: The system must keep quality gates green after each story/batch (`check`, `test`, `clippy -D warnings`, `fmt --check`).
- FR-5: Tolerance policy updates, if needed, must remain artifact-scoped with no global wildcard relaxation.
- FR-6: Migration docs must explicitly separate phase-1 completion (`runtime_support_dependency`) from phase-2 deferred scope (`out_of_scope`).

## 5. Non-Goals (Out of Scope)

- Migrating `out_of_scope` files in this PRD phase (currently `102` files).
- MPI runtime parity changes.
- New feature work outside migration and migration-governance tooling.
- Relaxing strict CI quality/parity failure behavior.

## 6. Design Considerations (Optional)

- Preserve existing module command interfaces (`rdinp`, `pot`, `xsph`, `path`, `fms`, `band`, `ldos`, `rixs`, `crpa`, `compton`, `ff2x`, `dmdw`, `screen`, `sfconv`, `eels`, `fullspectrum`).
- Keep artifact filenames and comparator contracts stable unless explicitly versioned and approved.
- Keep migration diffs focused; avoid unrelated broad refactors.

## 7. Technical Considerations (Optional)

- Primary implementation areas:
  - `crates/feff-core/src/modules/**`
  - `crates/feff-core/src/numerics/**`
  - `crates/feff-core/tests/*_parity.rs`
  - `crates/feff-cli/tests/regression_cli.rs`
  - `scripts/fortran/generate-gap-report.py`
  - `tasks/migration-gap-analysis.md`
  - `README.md`
  - `docs/developer-workflows.md`
- Required validation commands:
  - `scripts/fortran/ensure-feff10-reference.sh`
  - `cargo check --locked`
  - `cargo test --locked`
  - `cargo clippy --locked --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
  - workflow-equivalent oracle command to refresh `artifacts/regression/oracle-report.json`

## 8. Success Metrics

- `runtime_support_dependency` count becomes `0` in `gap/fortran-to-rust-file-gap.csv`.
- Existing oracle gate remains green: `passed=true`, `failed_fixture_count=0`, `mismatch_fixture_count=0`.
- Quality command set remains green for each batch.
- Documentation reflects phase-1 closure with explicit deferred phase-2 backlog.

## 9. Open Questions

- After phase-1 closure, should phase-2 (`out_of_scope`) be executed by physical Fortran directory waves or by runtime-module dependency backtracking?
- Should unresolved-target diagnostics in gap reporting be promoted from advisory to blocking criteria in phase-2?
