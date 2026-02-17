# PRD: FEFF10 True-Compute Rust Rewrite (Improved)

## 1. Introduction/Overview

Rewrite FEFF10 into Rust so all production scientific computation is executed by Rust code only. The rewrite allows aggressive refactoring, but must preserve user-visible contracts (CLI behavior, output artifact names/locations, diagnostics/exit codes) and scientific parity against Fortran within approved numeric tolerance policy.

This is a migration and modernization project, not a feature-expansion project.

## 2. Goals

- G-1: Deliver true-compute Rust implementations for all in-scope FEFF10 modules.
- G-2: Enforce zero Fortran runtime dependency in all production Rust execution paths.
- G-3: Preserve scientific equivalence under approved tolerance policy (`tasks/numeric-tolerance-policy.json`).
- G-4: Preserve compatibility contracts in `tasks/feff10-compatibility-matrix.md` and `tasks/migration-decision-log.md`.
- G-5: Keep migration decision-complete for autonomous implementation by defining execution order, fixture inventory, and acceptance gates.
- G-6: Enforce release-blocking dual-run oracle parity for migrated modules.

## 3. Scope

### 3.1 In Scope

- Full Rust rewrite for these modules: `RDINP`, `POT`, `PATH`, `FMS`, `XSPH`, `BAND`, `LDOS`, `RIXS`, `CRPA`, `COMPTON`, `DEBYE`, `DMDW`, `SCREEN`, `SELF`, `EELS`, `FULLSPECTRUM`.
- Aggressive architecture refactor for parser/numerics/pipelines/CLI boundaries.
- Oracle and snapshot parity tooling improvements for migration validation.
- CI gate hardening for quality + parity.
- Cutover and rollback rehearsal updates for true-compute runtime.

### 3.2 Out of Scope

- MPI distributed runtime parity in v1 (deferred by D-2).
- Net-new scientific models.
- GUI/web UI work.
- Production fallback execution through Fortran binaries/libraries.

### 3.3 Baseline-Copy Definition (Normative)

- Prohibited runtime behavior:
  - Any production module command (`feff`, `rdinp`, `pot`, etc.) reading `artifacts/fortran-baselines/**` to generate module outputs.
  - Any production module command copying baseline artifacts to satisfy output contracts.
- Permitted non-runtime behavior:
  - Regression comparison against baseline/oracle outputs.
  - Fixture/baseline generation via `scripts/fortran/capture-baselines.sh` and `scripts/fortran/generate-baseline-snapshots.sh`.
  - Test staging of fixture files in test-only paths.

## 4. Locked Contracts and Policy Inputs

### 4.1 Contract Artifacts

- Compatibility matrix: `tasks/feff10-compatibility-matrix.md`
- Numeric policy: `tasks/numeric-tolerance-policy.json`
- Fixture inventory: `tasks/golden-fixture-manifest.json`
- Module dependency map: `tasks/fortran-rust-boundary-map.md`
- Exit code + diagnostics contract: `tasks/migration-decision-log.md` (D-4)

### 4.2 Numeric Tolerance Policy (D-3)

Default comparison mode is `exact_text` with `first_match` category resolution. Numeric comparison rule:

`abs(actual-baseline) <= absTol || abs(actual-baseline) <= relTol * max(abs(baseline), relativeFloor)`

Approved numeric categories:

| Category | File patterns | absTol | relTol | relativeFloor |
| --- | --- | ---: | ---: | ---: |
| `columnar_spectra` | `xmu.dat`, `chi.dat`, `danes.dat`, `eels.dat`, `compton.dat`, `rixs*.dat` | `1e-8` | `1e-6` | `1e-12` |
| `density_tables` | `ldos*.dat`, `rho*.dat` | `5e-8` | `5e-6` | `1e-12` |
| `path_scattering_tables` | `feff*.dat`, `path*.dat` | `1e-7` | `1e-5` | `1e-12` |
| `thermal_workflow_tables` | `*.dmdw.out`, `debye*.dat`, `sig*.dat` | `1e-6` | `1e-4` | `1e-12` |

Exact-text categories remain exact (`diagnostic_logs`, `path_listing_reports`, `structured_reports`).

### 4.3 Exit Code and Diagnostics Contract (D-4)

Fatal exit mapping:

- `2`: `InputValidationError`
- `3`: `IoSystemError`
- `4`: `ComputationError`
- `5`: `InternalError`

Formatting contract:

- Warnings on `stderr` with prefix `WARNING: [<TOKEN>]`.
- Errors on `stderr` with prefix `ERROR: [<TOKEN>]`.
- Fatal termination appends exactly one line: `FATAL EXIT CODE: <n>`.

## 5. Approved Fixtures and Oracle Inventory

Fixture set from `tasks/golden-fixture-manifest.json` (17 fixtures total):

- `FX-RDINP-001`, `FX-POT-001`, `FX-PATH-001`, `FX-FMS-001`, `FX-XSPH-001`
- `FX-BAND-001`, `FX-LDOS-001`, `FX-RIXS-001`, `FX-CRPA-001`, `FX-COMPTON-001`
- `FX-DEBYE-001`, `FX-DMDW-001`, `FX-SCREEN-001`, `FX-SELF-001`, `FX-EELS-001`
- `FX-FULLSPECTRUM-001`, `FX-WORKFLOW-XAS-001`

Baseline status constraints:

- `requires_fortran_capture`: `FX-BAND-001`, `FX-FULLSPECTRUM-001`
- `reference_files_available`: `FX-RIXS-001`
- all remaining: `reference_archive_available`

Fixture approval rule:

- A fixture is "approved" only when listed in manifest with `comparison.passFailThreshold` set to `minimumArtifactPassRate=1.0`, `maxArtifactFailures=0`.
- New fixtures must be added to manifest + baseline snapshot + checksum metadata in one change set.

## 6. Dependency-Safe Story Execution Order

Execution sequence is mandatory for implementation:

1. US-001 through US-006 (contracts, architecture, oracle infra, parser, numerics, IO).
2. Core chain:
   - US-007 (`RDINP`)
   - US-008 (`POT`)
   - US-009 (`SCREEN`) and US-010 (`CRPA`) in parallel
   - US-011 (`XSPH`)
   - US-012 (`PATH`) and US-013 (`FMS`) in parallel
3. Core-adjacent:
   - US-014 (`BAND`)
   - US-015 (`LDOS`)
   - US-016 (`COMPTON`)
   - US-017 (`DEBYE`)
   - US-018 (`DMDW`)
4. Spectroscopy/post-processing:
   - US-019 (`SELF`)
   - US-020 (`EELS`)
   - US-021 (`FULLSPECTRUM`)
   - US-022 (`RIXS`)
5. Cross-cutting completion:
   - US-023 through US-026.

## 7. User Stories

### US-001: Freeze migration contracts
**Description:** As a migration lead, I want contract artifacts and decision references locked so all module rewrites use one authoritative spec.

**Acceptance Criteria:**
- [ ] PRD references and aligns with: compatibility matrix, fixture manifest, tolerance policy, D-4 contract.
- [ ] Baseline-copy prohibited behavior is explicitly defined and testable.
- [ ] Story sequence dependencies are documented and ordered.
- [ ] Tests pass for contract validation tooling updates (if touched).
- [ ] Typecheck passes.

### US-002: Define target Rust architecture for aggressive refactor
**Description:** As a maintainer, I want a refactored architecture with explicit module boundaries so long-term evolution is sustainable.

**Acceptance Criteria:**
- [ ] Architecture defines boundaries for `domain`, `parser`, `numerics`, `pipelines`, and `cli`.
- [ ] Shared typed contracts are defined for module request/response, artifacts, and errors.
- [ ] Architecture document includes migration guidance for removing baseline-copy runtime paths.
- [ ] Tests pass for any new shared primitives.
- [ ] Typecheck passes.

### US-003: Build dual-run oracle harness (validation-only path)
**Description:** As a release engineer, I want deterministic Rust-vs-Fortran oracle comparison so release parity is enforceable.

**Acceptance Criteria:**
- [ ] Oracle harness supports running Fortran captures with `scripts/fortran/capture-baselines.sh`.
- [ ] Snapshot generation supports all fixtures with `scripts/fortran/generate-baseline-snapshots.sh`.
- [ ] Oracle comparison outputs both JSON report and human-readable diff.
- [ ] Oracle tooling is not used by production module execution commands.
- [ ] Tests pass for touched scripts/tooling.
- [ ] Typecheck passes.

### US-004: Port parser to complete typed FEFF model
**Description:** As a module implementer, I want a parser that covers fixture-backed FEFF cards so compute engines consume stable typed data.

**Acceptance Criteria:**
- [ ] Parser accepts all manifest fixture entry decks for in-scope modules.
- [ ] Parser emits deterministic validation errors with D-4-compliant categories.
- [ ] Snapshot tests cover accepted decks and representative invalid decks.
- [ ] Tests pass.
- [ ] Typecheck passes.

### US-005: Port numerics foundation with D-3 policy alignment
**Description:** As a module implementer, I want deterministic numeric primitives so module outputs remain reproducible and policy-comparable.

**Acceptance Criteria:**
- [ ] Shared numeric utilities cover interpolation, stable summation, sorting, and tolerance comparison primitives used by modules.
- [ ] Numeric parser supports Fortran exponent markers (`D`, `d`).
- [ ] NaN/Inf mismatch policy behavior is implemented and tested.
- [ ] Tests pass.
- [ ] Typecheck passes.

### US-006: Implement deterministic artifact I/O and serialization layer
**Description:** As a module implementer, I want shared artifact readers/writers so text/binary outputs remain contract-compliant.

**Acceptance Criteria:**
- [ ] Shared I/O layer handles required text and binary artifact formats per module contracts.
- [ ] Deterministic write behavior is guaranteed on repeated same-input runs.
- [ ] Binary contract policy is explicit: same-target byte-identical for downstream-consumed binaries (`pot.bin`, `phase.bin`, `gg.bin`, `feff.dym`).
- [ ] Tests pass.
- [ ] Typecheck passes.

### US-007: Rewrite RDINP true-compute module
**Description:** As a user, I want `RDINP` implemented in Rust so downstream decks are generated without Fortran runtime dependency.

**Inputs:** `feff.inp`  
**Required outputs:** `geom.dat`, `global.inp`, `reciprocal.inp`, `pot.inp`, `ldos.inp`, `xsph.inp`, `fms.inp`, `paths.inp`, `genfmt.inp`, `ff2x.inp`, `sfconv.inp`, `eels.inp`, `compton.inp`, `band.inp`, `rixs.inp`, `crpa.inp`, `fullspectrum.inp`, `dmdw.inp`, `log.dat` (+ `screen.inp` when `SCREEN` card exists)  
**Fixtures:** `FX-RDINP-001`, `FX-WORKFLOW-XAS-001`

**Acceptance Criteria:**
- [ ] Rust `RDINP` generates all required downstream artifacts from parsed FEFF input.
- [ ] Output structure and card defaults follow compatibility matrix contracts.
- [ ] Oracle parity passes on both listed fixtures.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-008: Rewrite POT true-compute module
**Description:** As a user, I want `POT` computed natively in Rust so potential artifacts are not baseline-copied.

**Inputs:** `pot.inp`, `geom.dat`  
**Required outputs:** `pot.bin`, `pot.dat`, `log1.dat`, `convergence.scf`, `convergence.scf.fine`  
**Fixtures:** `FX-POT-001`, `FX-WORKFLOW-XAS-001`

**Acceptance Criteria:**
- [ ] Rust `POT` computes outputs from module inputs without reading baseline output artifacts.
- [ ] Output files match compatibility matrix naming/location contract.
- [ ] Oracle parity passes on listed fixtures.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-009: Rewrite SCREEN true-compute module
**Description:** As a user, I want `SCREEN` computed in Rust so screened-core-hole artifacts are generated by native code.

**Inputs:** `pot.inp`, `geom.dat`, `ldos.inp`, optional `screen.inp`  
**Required outputs:** `wscrn.dat`, `logscreen.dat`  
**Fixtures:** `FX-SCREEN-001`

**Acceptance Criteria:**
- [ ] Rust `SCREEN` supports optional override input semantics and required default behavior.
- [ ] Output contract and diagnostics follow compatibility and D-4 rules.
- [ ] Oracle parity passes on listed fixture.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-010: Rewrite CRPA true-compute module
**Description:** As a user, I want `CRPA` computed in Rust so screening artifacts are generated natively.

**Inputs:** `crpa.inp`, `pot.inp`, `geom.dat`  
**Required outputs:** `wscrn.dat`, `logscrn.dat`  
**Fixtures:** `FX-CRPA-001`

**Acceptance Criteria:**
- [ ] Rust `CRPA` computes screening outputs from declared inputs.
- [ ] Output and error behavior match compatibility contract.
- [ ] Oracle parity passes on listed fixture.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-011: Rewrite XSPH true-compute module
**Description:** As a user, I want `XSPH` computed in Rust so phase and cross-section outputs are generated natively.

**Inputs:** `xsph.inp`, `geom.dat`, `global.inp`, `pot.bin`, optional `wscrn.dat`  
**Required outputs:** `phase.bin`, `xsect.dat`, `log2.dat`, optional `phase.dat`  
**Fixtures:** `FX-XSPH-001`, `FX-WORKFLOW-XAS-001`

**Acceptance Criteria:**
- [ ] Rust `XSPH` handles both screened and unscreened workflows correctly.
- [ ] Binary/text artifact formats satisfy downstream module contracts.
- [ ] Oracle parity passes on listed fixtures.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-012: Rewrite PATH true-compute module
**Description:** As a user, I want `PATH` computed in Rust so path enumeration and ordering are native.

**Inputs:** `paths.inp`, `geom.dat`, `global.inp`, `phase.bin`  
**Required outputs:** `paths.dat`, `paths.bin`, `crit.dat`, `log4.dat`  
**Fixtures:** `FX-PATH-001`, `FX-WORKFLOW-XAS-001`

**Acceptance Criteria:**
- [ ] Rust `PATH` computes path listings with compatibility ordering/filter semantics.
- [ ] `paths.dat` exact-text behavior is preserved under policy.
- [ ] Oracle parity passes on listed fixtures.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-013: Rewrite FMS true-compute module
**Description:** As a user, I want `FMS` computed in Rust so scattering outputs are generated natively.

**Inputs:** `fms.inp`, `geom.dat`, `global.inp`, `phase.bin`  
**Required outputs:** `gg.bin`, `log3.dat`  
**Fixtures:** `FX-FMS-001`, `FX-WORKFLOW-XAS-001`

**Acceptance Criteria:**
- [ ] Rust `FMS` computes required outputs without baseline materialization.
- [ ] Binary format required by downstream consumers is preserved.
- [ ] Oracle parity passes on listed fixtures.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-014: Rewrite BAND true-compute module
**Description:** As a user, I want `BAND` computed in Rust so bandstructure artifacts are native.

**Inputs:** `band.inp`, `geom.dat`, `global.inp`, `phase.bin`  
**Required outputs:** `bandstructure.dat`, `logband.dat`  
**Fixtures:** `FX-BAND-001`

**Acceptance Criteria:**
- [ ] Rust `BAND` computes required outputs from staged inputs with no baseline-copy runtime behavior.
- [ ] Fixture capture path handles `requires_fortran_capture` status for oracle generation.
- [ ] Oracle parity passes on listed fixture.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-015: Rewrite LDOS true-compute module
**Description:** As a user, I want `LDOS` computed in Rust so DOS tables are generated natively.

**Inputs:** `ldos.inp`, `geom.dat`, `pot.bin`, `reciprocal.inp`  
**Required outputs:** `ldos00.dat`, `ldosNN.dat` family, `logdos.dat`  
**Fixtures:** `FX-LDOS-001`

**Acceptance Criteria:**
- [ ] Rust `LDOS` computes required DOS output families according to contract.
- [ ] Numeric tolerance comparison passes for density-table category.
- [ ] Oracle parity passes on listed fixture.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-016: Rewrite COMPTON true-compute module
**Description:** As a user, I want `COMPTON` computed in Rust so compton-domain outputs are generated natively.

**Inputs:** `compton.inp`, `pot.bin`, `gg_slice.bin`  
**Required outputs:** `compton.dat`, `jzzp.dat`, `rhozzp.dat`, `logcompton.dat`  
**Fixtures:** `FX-COMPTON-001`

**Acceptance Criteria:**
- [ ] Rust `COMPTON` supports mixed binary/text input handling.
- [ ] Required output set is fully generated and contract-compliant.
- [ ] Oracle parity passes on listed fixture.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-017: Rewrite DEBYE true-compute module
**Description:** As a user, I want `DEBYE` computed in Rust so Debye-Waller outputs are generated natively.

**Inputs:** `ff2x.inp`, `paths.dat`, `feff.inp`, optional `spring.inp`  
**Required outputs:** `s2_em.dat`, `s2_rm1.dat`, `s2_rm2.dat`, updated `xmu.dat`/`chi.dat`, `log6.dat`  
**Fixtures:** `FX-DEBYE-001`

**Acceptance Criteria:**
- [ ] Rust `DEBYE` handles optional spring-driven behavior correctly.
- [ ] Required output family is generated with compatible behavior.
- [ ] Oracle parity passes on listed fixture.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-018: Rewrite DMDW true-compute module
**Description:** As a user, I want `DMDW` computed in Rust so dynamic Debye outputs are generated natively.

**Inputs:** `dmdw.inp`, `feff.dym`  
**Required outputs:** `dmdw.out`  
**Fixtures:** `FX-DMDW-001`

**Acceptance Criteria:**
- [ ] Rust `DMDW` computes output without baseline materialization.
- [ ] Binary/text mixed input handling preserves contract behavior.
- [ ] Oracle parity passes on listed fixture.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-019: Rewrite SELF true-compute module
**Description:** As a user, I want `SELF` computed in Rust so self-energy artifacts are generated natively.

**Inputs:** `sfconv.inp`, at least one of `xmu.dat`/`chi.dat`/`feffNNNN.dat`, optional `exc.dat`  
**Required outputs:** `selfenergy.dat`, `sigma.dat`, `specfunct.dat`, rewritten spectra, `logsfconv.dat`  
**Fixtures:** `FX-SELF-001`

**Acceptance Criteria:**
- [ ] Rust `SELF` enforces one-of required spectrum input contract.
- [ ] Required outputs are generated with compatibility-compliant naming/format.
- [ ] Oracle parity passes on listed fixture.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-020: Rewrite EELS true-compute module
**Description:** As a user, I want `EELS` computed in Rust so EELS outputs are generated natively.

**Inputs:** `eels.inp`, `xmu.dat`  
**Required outputs:** `eels.dat`, optional `magic.dat`, `logeels.dat`  
**Fixtures:** `FX-EELS-001`

**Acceptance Criteria:**
- [ ] Rust `EELS` computes required outputs including optional magic-angle flow when enabled.
- [ ] Numeric/text behavior follows policy and compatibility contracts.
- [ ] Oracle parity passes on listed fixture.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-021: Rewrite FULLSPECTRUM true-compute module
**Description:** As a user, I want `FULLSPECTRUM` computed in Rust so merged-spectrum outputs are generated natively.

**Inputs:** `fullspectrum.inp`, component `xmu.dat` paths (`fms_re/`, `fms_im/`, `path_re/`, `path_im/`, `fprime*/`)  
**Required outputs:** `xmu.dat`, `osc_str.dat`, `eps.dat`, `drude.dat`, `background.dat`, `fine_st.dat`, `logfullspectrum.dat`  
**Fixtures:** `FX-FULLSPECTRUM-001`

**Acceptance Criteria:**
- [ ] Rust `FULLSPECTRUM` computes required outputs without baseline output copy behavior.
- [ ] Fixture capture path handles `requires_fortran_capture` status for oracle generation.
- [ ] Oracle parity passes on listed fixture.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-022: Rewrite RIXS true-compute module
**Description:** As a user, I want `RIXS` computed in Rust so multi-edge RIXS outputs are generated natively.

**Inputs:** `rixs.inp`, `phase_1.bin`, `phase_2.bin`, `wscrn_1.dat`, `wscrn_2.dat`, `xsect_2.dat`  
**Required outputs:** `rixs0.dat`, `rixs1.dat`, `rixsET.dat`, `rixsEE.dat`, `rixsET-sat.dat`, `rixsEE-sat.dat`, `logrixs.dat`  
**Fixtures:** `FX-RIXS-001`

**Acceptance Criteria:**
- [ ] Rust `RIXS` supports multi-edge staged input contracts and output variants.
- [ ] Special fixture mode for reference-file baselines is handled correctly.
- [ ] Oracle parity passes on listed fixture.
- [ ] Module tests pass.
- [ ] Typecheck passes.

### US-023: Remove baseline-copy runtime behavior from all modules
**Description:** As a maintainer, I want baseline-copy behavior removed from runtime so all outputs come from Rust compute engines.

**Acceptance Criteria:**
- [ ] Runtime module commands do not read `artifacts/fortran-baselines/**` for output generation.
- [ ] Regression tooling retains baseline/oracle comparisons as validation-only behavior.
- [ ] Guard tests fail if runtime baseline-copy behavior is reintroduced.
- [ ] Tests pass.
- [ ] Typecheck passes.

### US-024: Harden CI gates for quality and parity
**Description:** As a release owner, I want strict CI gates so only compatibility-safe changes merge.

**Acceptance Criteria:**
- [ ] Quality gates run `cargo check --locked`, `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, and `cargo fmt --all -- --check`.
- [ ] Parity workflow requires oracle comparison for migrated modules on protected branches.
- [ ] Failure artifacts include JSON report + human-readable diff.
- [ ] Tests pass for touched CI tooling.
- [ ] Typecheck passes.

### US-025: Rehearse cutover and rollback with true-compute binaries
**Description:** As an operator, I want rehearsed operational flows so release risk is controlled.

**Acceptance Criteria:**
- [ ] Cutover rehearsal validates required workflow outputs using true-compute Rust binaries.
- [ ] Rollback rehearsal validates switching to last stable Fortran bundle and recovery procedure.
- [ ] Rehearsal reports are stored under `tasks/` with run date and evidence links.
- [ ] Tests pass for touched scripts/tooling.
- [ ] Typecheck passes.

### US-026: Publish migration operator/developer documentation updates
**Description:** As a maintainer, I want docs aligned with true-compute architecture so users and developers operate the system correctly.

**Acceptance Criteria:**
- [ ] Developer workflow docs clearly distinguish runtime execution from oracle validation paths.
- [ ] Operator docs document known limits (including deferred MPI parity).
- [ ] Troubleshooting docs include deterministic guidance for oracle capture and parity failures.
- [ ] Documentation checks/tests pass where applicable.
- [ ] Typecheck passes.

## 8. Functional Requirements

- FR-1: Production scientific module execution must be Rust-only.
- FR-2: Production commands must not call Fortran binaries/libraries.
- FR-3: Runtime behavior must preserve CLI contracts in the compatibility matrix.
- FR-4: Runtime behavior must preserve output artifact names and output locations in the compatibility matrix.
- FR-5: Runtime diagnostics must follow D-4 prefix and stream contract.
- FR-6: Fatal runtime errors must map to D-4 exit-code categories (`2`, `3`, `4`, `5`).
- FR-7: Comparator must load `tasks/numeric-tolerance-policy.json` and apply first-match category resolution.
- FR-8: Comparator must support Fortran exponent markers (`D`, `d`) during numeric parsing.
- FR-9: NaN/Inf mismatches, line-count mismatches, and token-count mismatches are hard failures.
- FR-10: Fixture pass threshold is strict (`minimumArtifactPassRate=1.0`, `maxArtifactFailures=0`) unless explicitly changed in manifest.
- FR-11: Module implementation order must follow Section 6 dependency order.
- FR-12: `RDINP` must generate all downstream stage input artifacts listed in US-007.
- FR-13: `POT` must compute and write all required artifacts listed in US-008.
- FR-14: `SCREEN` and `CRPA` must compute required screening outputs and optional-input behavior.
- FR-15: `XSPH` must generate phase/xsect outputs and support optional `wscrn.dat`.
- FR-16: `PATH` must preserve ordering/filter semantics for path outputs.
- FR-17: `FMS` must preserve required binary output compatibility for downstream use.
- FR-18: `BAND`, `LDOS`, `COMPTON`, `DEBYE`, `DMDW`, `SELF`, `EELS`, `FULLSPECTRUM`, and `RIXS` must each compute required output sets listed in stories.
- FR-19: Runtime pipelines must not copy baseline artifacts to satisfy outputs.
- FR-20: Oracle tooling must remain outside production runtime code paths.
- FR-21: Oracle and snapshot generation must support all fixtures in manifest.
- FR-22: CI must publish report artifacts on parity failure.
- FR-23: CI merge gates must fail on parity regressions for migrated modules.
- FR-24: MPI parity remains deferred in v1 runtime scope and must be documented as unsupported for distributed execution.
- FR-25: Any intentional compatibility deviation requires documented approval before release.

## 9. Non-Goals (Out of Scope)

- MPI-distributed computational parity in v1.
- New scientific algorithms beyond FEFF10 behavior preservation.
- UI/front-end features.
- Bitwise-identical outputs across all hardware/OS combinations.

## 10. Design Considerations

- Prefer explicit typed contracts over implicit file-driven coupling.
- Keep module interfaces small and deterministic.
- Keep binary/text serializers centralized and reusable.
- Maintain strict separation between runtime pipelines and parity/oracle tooling.

## 11. Technical Considerations

- Determinism guarantee scope:
  - Exact deterministic behavior is required on GA targets from D-1 (`x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`).
  - Cross-target numeric drift is accepted only within D-3 tolerances.
- Oracle infrastructure:
  - Oracle capture is performed by `scripts/fortran/capture-baselines.sh`.
  - Baseline snapshot materialization is performed by `scripts/fortran/generate-baseline-snapshots.sh`.
  - Fortran toolchain availability is required only in oracle-validation environments, not in primary Rust runtime build/test path.
- Binary format policy:
  - For binary artifacts consumed by downstream modules, preserve same-target byte compatibility.
  - Cross-target parity uses policy comparison on text/numeric outputs and contract checks for binary presence/consumption behavior.
- Performance requirement:
  - No approved workflow may exceed `1.10x` the Fortran baseline runtime on GA targets.
  - Target median runtime improvement of at least `10%` on top workflow fixture set by GA+1 iteration.

## 12. Success Metrics

- SM-1: 100% of in-scope modules run true-compute Rust paths in production.
- SM-2: 0 production runtime invocations of Fortran binaries/libraries.
- SM-3: 100% pass rate for oracle parity on approved fixtures for migrated modules.
- SM-4: 100% pass rate on CI quality gates (`check`, `test`, `clippy`, `fmt`).
- SM-5: 0 critical compatibility regressions in cutover rehearsal.
- SM-6: 0 rollback rehearsal failures when switching to stable Fortran bundle.
- SM-7: Runtime performance stays within `<=1.10x` Fortran baseline for all approved workflows.

## 13. Open Questions

- OQ-1: What delivery timeline and staffing allocation is approved for this full-module rewrite?
- OQ-2: Which fixture additions should be prioritized first beyond the current 17-fixture set after all module rewrites pass?

