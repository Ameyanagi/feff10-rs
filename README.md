# feff10-rs

## Documentation

- Migration status summary (release-blocking): `tasks/oracle-16-module-migration-summary.md`
- Migration closeout analysis: `tasks/migration-gap-analysis.md`
- Developer workflows: `docs/developer-workflows.md`
- User and operator guide: `docs/operator-guide.md`
- Troubleshooting: `docs/troubleshooting.md`
- License summary: `LICENSE`
- Repository licenses: `LICENSE-MIT`, `LICENSE-APACHE`
- FEFF10 third-party license: `THIRD_PARTY_LICENSES/FEFF10-LICENSE`
- Cutover rehearsal checklist: `docs/cutover-rehearsal-checklist.md`
- Rollback rehearsal checklist: `docs/rollback-rehearsal-checklist.md`
- Latest cutover rehearsal report: `tasks/cutover-rehearsal-2026-02-18.md`
- Latest rollback rehearsal and GA sign-off report: `tasks/rollback-rehearsal-2026-02-18.md`

## License And Attribution

This repository's original Rust code is dual-licensed under `MIT` or `Apache-2.0` at your
option. See `LICENSE-MIT`, `LICENSE-APACHE`, and `LICENSE`.

FEFF10 reference code and example inputs are not authored by this repository owner and are not
relicensed under MIT/Apache by this project. FEFF10 remains under its original license and
copyright:

- FEFF Project, University of Washington, and SLAC National Accelerator Laboratory
- Upstream project: `https://github.com/times-software/feff10`
- License terms: `THIRD_PARTY_LICENSES/FEFF10-LICENSE`

Any use or redistribution of FEFF10-derived materials must comply with the FEFF10 license terms.

## Rust Architecture

The Rust workspace keeps module boundaries explicit across `feff-core` and `feff-cli`:

- `crates/feff-core/src/domain`: shared FEFF-domain types and execution request models
- `crates/feff-core/src/parser`: FEFF input deck tokenizer/parser entrypoint
- `crates/feff-core/src/numerics`: shared numeric helper primitives
- `crates/feff-core/src/modules`: module pipelines, regression, and comparator infrastructure
- `crates/feff-cli/src`: CLI command parsing and orchestration

## Rust Quality Gates

Local commands matching CI checks:

```bash
scripts/fortran/ensure-feff10-reference.sh
cargo check --locked
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
```

`cargo test` and oracle parity flows read fixture inputs from `feff10/examples/...`.
`feff10/` is intentionally not committed in this repository; it is a local checkout fetched by
`scripts/fortran/ensure-feff10-reference.sh`. FEFF10 usage must follow
`THIRD_PARTY_LICENSES/FEFF10-LICENSE`.

macOS targets are configured to use `clang` by default via `.cargo/config.toml`.
If your local toolchain still cannot resolve `clang`, install Xcode Command Line Tools or run:

```bash
xcrun -f clang
```

## Rust Parity Gates

Local command flow matching `.github/workflows/rust-parity-gates.yml`:

```bash
scripts/fortran/ensure-feff10-reference.sh
mkdir -p artifacts/regression
cargo run --locked -- oracle \
  --manifest tasks/golden-fixture-manifest.json \
  --policy tasks/numeric-tolerance-policy.json \
  --oracle-root artifacts/fortran-oracle-capture \
  --oracle-subdir outputs \
  --actual-root artifacts/fortran-baselines \
  --actual-subdir baseline \
  --report artifacts/regression/oracle-report.json \
  --capture-runner "./scripts/fortran/ci-oracle-capture-runner.sh" \
  --capture-allow-missing-entry-files \
  > artifacts/regression/oracle-summary.txt \
  2> artifacts/regression/oracle-stderr.txt

jq -r '
  def status(v): if v then "PASS" else "FAIL" end;
  "Oracle parity status: \(status(.passed))",
  "Failed fixtures: \(.failed_fixture_count)",
  "Failed artifacts: \(.failed_artifact_count)",
  "Mismatched fixtures: \(.mismatch_fixture_count // 0)",
  "Mismatched artifacts: \(.mismatch_artifact_count // 0)",
  "",
  "Mismatched artifact details:",
  (
    (.mismatch_fixtures // [])[]
    | "Fixture \(.fixture_id):",
      (
        .artifacts[]
        | "  - \(.artifact_path): \(.reason // \"comparison failed\")"
      )
  )
' artifacts/regression/oracle-report.json > artifacts/regression/oracle-diff.txt
```

For BAND oracle validation on `FX-BAND-001` (`baselineStatus=requires_fortran_capture`), include
`--capture-allow-missing-entry-files` so unresolved `REFERENCE/band.inp` is recorded in capture
metadata and the parity run can continue.

When the oracle command exits non-zero, CI uploads:
- `artifacts/regression/oracle-report.json`
- `artifacts/regression/oracle-diff.txt`
- `artifacts/regression/oracle-summary.txt`
- `artifacts/regression/oracle-stderr.txt`

## CLI Compatibility Commands

The binary now exposes FEFF-compatible command surfaces in addition to `regression`:

```bash
cargo run -- feff
cargo run -- feffmpi 4
cargo run -- oracle --help
cargo run -- rdinp
cargo run -- pot
cargo run -- xsph
```

Supported module commands are:

- `rdinp`
- `pot`
- `xsph`
- `path`
- `fms`
- `band`
- `ldos`
- `rixs`
- `crpa`
- `compton`
- `ff2x` (DEBYE)
- `dmdw`
- `screen`
- `sfconv` (SELF)
- `eels`
- `fullspectrum`

All module commands run in the current working directory and do not accept positional arguments.
Runtime commands (`feff`, `feffmpi`, and module commands) must not use `artifacts/fortran-baselines` as output-generation sources.
Baseline snapshots are validation/test-only inputs for regression and fixture tooling.
Runtime compute engines are currently available for `RDINP`, `POT`, `SCREEN`, `SELF`, `EELS`, `FULLSPECTRUM`, `CRPA`, `XSPH`, `PATH`, `FMS`, `BAND`, `LDOS`, `RIXS`, `COMPTON`, `DEBYE`, and `DMDW`.

MPI parity is still deferred for Rust v1 (`D-2`). `feffmpi <nprocs>` validates `<nprocs>` and runs the serial compatibility chain, emitting a deterministic warning when `nprocs > 1`.

## Fortran Baseline Snapshots

Regenerate committed fixture baselines and checksum metadata:

```bash
scripts/fortran/generate-baseline-snapshots.sh \
  --manifest tasks/golden-fixture-manifest.json \
  --output-root artifacts/fortran-baselines
```

Prerequisites:
- `bash`
- `jq`
- `unzip`
- `sha256sum` or `shasum`

Optional execution modes:
- `--capture-runner "<command>"` to run a custom capture command per fixture.
- `--capture-bin-dir <path>` to run Fortran module executables directly.

## Regression Runner

Run all fixture comparisons in one command and emit a JSON report:

```bash
cargo run -- regression \
  --manifest tasks/golden-fixture-manifest.json \
  --policy tasks/numeric-tolerance-policy.json \
  --baseline-root artifacts/fortran-baselines \
  --actual-root artifacts/fortran-baselines \
  --baseline-subdir baseline \
  --actual-subdir baseline \
  --report artifacts/regression/report.json
```

The command prints a human-readable pass/fail summary and exits with status `1` when any fixture fails.
Use `--run-rdinp` when you want the Rust RDINP pipeline to materialize outputs into `--actual-root/<fixture>/<actual-subdir>` before comparisons.
Use `--run-pot` to execute the Rust POT true-compute path before comparisons; it expects staged `pot.inp` and `geom.dat` in each fixture actual output directory and computes the POT artifact contract (`pot.bin`, `pot.dat`, `log1.dat`, `convergence.scf`, `convergence.scf.fine`) without baseline snapshot reads.
Use `--run-screen` to execute the Rust SCREEN true-compute path before comparisons; it expects staged `pot.inp`, `geom.dat`, and `ldos.inp` (optionally `screen.inp`) in each fixture actual output directory and computes the SCREEN artifact contract (`wscrn.dat`, `logscreen.dat`) without baseline snapshot reads.
Use `--run-xsph` to execute the Rust XSPH true-compute path before comparisons; it expects staged `xsph.inp`, `geom.dat`, `global.inp`, and `pot.bin` (optionally `wscrn.dat`) in each fixture actual output directory and computes the XSPH artifact contract (`phase.bin`, `xsect.dat`, `log2.dat`) without baseline snapshot reads.
Use `--run-path` to execute the Rust PATH true-compute path before comparisons; it expects staged `paths.inp`, `geom.dat`, `global.inp`, and `phase.bin` in each fixture actual output directory and computes the PATH artifact contract (`paths.dat`, `paths.bin`, `crit.dat`, `log4.dat`) without baseline snapshot reads.
Use `--run-fms` to execute the Rust FMS true-compute path before comparisons; it expects staged `fms.inp`, `geom.dat`, `global.inp`, and `phase.bin` in each fixture actual output directory and computes the FMS artifact contract (`gg.bin`, `log3.dat`) without baseline snapshot reads.
Use `--run-band` to execute the Rust BAND true-compute path before comparisons; it expects staged `band.inp`, `geom.dat`, `global.inp`, and `phase.bin` in each fixture actual output directory and computes the BAND artifact contract (`bandstructure.dat`, `logband.dat`) without baseline snapshot reads.
Use `--run-ldos` to execute the Rust LDOS true-compute path before comparisons; it expects staged `ldos.inp`, `geom.dat`, `pot.bin`, and `reciprocal.inp` in each fixture actual output directory and computes the LDOS artifact contract (`ldos*.dat` series and `logdos.dat`) without baseline snapshot reads.
Use `--run-rixs` to execute the Rust RIXS true-compute path before comparisons; it expects staged `rixs.inp`, `phase_1.bin`, `phase_2.bin`, `wscrn_1.dat`, `wscrn_2.dat`, and `xsect_2.dat` in each fixture actual output directory and computes the RIXS artifact contract (`rixs0.dat`, `rixs1.dat`, `rixsET.dat`, `rixsEE.dat`, `rixsET-sat.dat`, `rixsEE-sat.dat`, `logrixs.dat`) without baseline snapshot reads.
Use `--run-crpa` to execute the Rust CRPA true-compute path before comparisons; it expects staged `crpa.inp`, `pot.inp`, and `geom.dat` in each fixture actual output directory and computes the CRPA artifact contract (`wscrn.dat`, `logscrn.dat`) without baseline snapshot reads.
Use `--run-compton` to execute the Rust COMPTON true-compute path before comparisons; it expects staged `compton.inp`, `pot.bin`, and `gg_slice.bin` in each fixture actual output directory and computes the COMPTON artifact contract (`compton.dat`, `jzzp.dat`, `rhozzp.dat`, `logcompton.dat`) without baseline snapshot reads.
Use `--run-debye` to execute the Rust DEBYE true-compute path before comparisons; it expects staged `ff2x.inp`, `paths.dat`, and `feff.inp` (optionally `spring.inp`) in each fixture actual output directory and computes the DEBYE artifact contract (`s2_em.dat`, `s2_rm1.dat`, `s2_rm2.dat`, `xmu.dat`, `chi.dat`, `log6.dat`, `spring.dat`) without baseline snapshot reads.
Use `--run-dmdw` to execute the Rust DMDW true-compute path before comparisons; it expects staged `dmdw.inp` and `feff.dym` in each fixture actual output directory and computes the DMDW artifact contract (`dmdw.out`) without baseline snapshot reads.
Use `--run-self` to execute the Rust SELF true-compute path before comparisons; it expects staged `sfconv.inp` and at least one spectrum input (`xmu.dat`, `chi.dat`, `loss.dat`, or `feffNNNN.dat`) plus optional `exc.dat` in each fixture actual output directory and computes the SELF artifact contract (`selfenergy.dat`, `sigma.dat`, `specfunct.dat`, `logsfconv.dat`, `sig2FEFF.dat`, `mpse.dat`, `opconsCu.dat`, plus rewritten staged spectrum artifacts) without baseline snapshot reads.
Use `--run-eels` to execute the Rust EELS true-compute path before comparisons; it expects staged `eels.inp` and `xmu.dat` (optionally `magic.inp`) in each fixture actual output directory and computes the EELS artifact contract (`eels.dat`, `logeels.dat`, optional `magic.dat`) without baseline snapshot reads.
Use `--run-fullspectrum` to execute the Rust FULLSPECTRUM true-compute path before comparisons; it expects staged `fullspectrum.inp` and `xmu.dat` (optionally `prexmu.dat` and `referencexmu.dat`) in each fixture actual output directory and computes the FULLSPECTRUM artifact contract (`xmu.dat`, `osc_str.dat`, `eps.dat`, `drude.dat`, `background.dat`, `fine_st.dat`, `logfullspectrum.dat`) without baseline snapshot reads.

## Oracle Dual-Run Validation

Run Fortran oracle capture and Rust parity comparison as one validation-only command:

```bash
cargo run -- oracle \
  --manifest tasks/golden-fixture-manifest.json \
  --policy tasks/numeric-tolerance-policy.json \
  --oracle-root artifacts/fortran-oracle-capture \
  --oracle-subdir outputs \
  --actual-root artifacts/oracle-actual \
  --actual-subdir actual \
  --report artifacts/regression/oracle-report.json \
  --capture-runner "<fortran capture command>" \
  --run-rdinp
```

Notes:
- Use exactly one capture mode: `--capture-runner "<command>"` or `--capture-bin-dir <path>`.
- The command always captures the same manifest fixture set that the regression comparison evaluates.
- `oracle` is a validation-only path and must not be used by production runtime commands (`feff`, `feffmpi`, module commands).
