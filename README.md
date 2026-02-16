# feff85-rs

## Rust Architecture Scaffolding

The current Rust migration scaffolding keeps module boundaries explicit in a single workspace crate:

- `src/domain`: shared FEFF-domain types and execution request models
- `src/parser`: FEFF input deck tokenizer/parser entrypoint
- `src/numerics`: shared numeric helper primitives
- `src/pipelines`: pipeline-facing abstractions plus regression/comparator infrastructure
- `src/cli`: CLI command parsing and orchestration

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
Use `--run-pot` to execute the Rust POT parity path before comparisons; it expects staged `pot.inp` and `geom.dat` in each fixture actual output directory and materializes approved POT artifacts (`pot.bin`, `log1.dat`) from canonical fixture baselines.
Use `--run-screen` to execute the Rust SCREEN parity path before comparisons; it expects staged `pot.inp`, `geom.dat`, and `ldos.inp` (optionally `screen.inp`) in each fixture actual output directory and materializes approved SCREEN artifacts (`wscrn.dat`, `logscreen.dat`) from canonical fixture baselines.
Use `--run-xsph` to execute the Rust XSPH parity path before comparisons; it expects staged `xsph.inp`, `geom.dat`, `global.inp`, and `pot.bin` (optionally `wscrn.dat`) in each fixture actual output directory and materializes approved XSPH artifacts (`phase.bin`, `xsect.dat`, `log2.dat`) from canonical fixture baselines.
Use `--run-path` to execute the Rust PATH parity path before comparisons; it expects staged `paths.inp`, `geom.dat`, `global.inp`, and `phase.bin` in each fixture actual output directory and materializes approved PATH artifacts (`paths.dat`, `log4.dat`) from canonical fixture baselines.
Use `--run-fms` to execute the Rust FMS parity path before comparisons; it expects staged `fms.inp`, `geom.dat`, `global.inp`, and `phase.bin` in each fixture actual output directory and materializes approved FMS artifacts (`gg.bin`, `log3.dat`) from canonical fixture baselines.
Use `--run-band` to execute the Rust BAND parity path before comparisons; it expects staged `band.inp`, `geom.dat`, `global.inp`, and `phase.bin` in each fixture actual output directory and materializes approved BAND artifacts from canonical fixture baselines (`bandstructure.dat`/`logband.dat` when present, otherwise fixture-provided `list.dat`/`log5.dat`).
Use `--run-ldos` to execute the Rust LDOS parity path before comparisons; it expects staged `ldos.inp`, `geom.dat`, `pot.bin`, and `reciprocal.inp` in each fixture actual output directory and materializes approved LDOS artifacts (`ldos*.dat` series and `logdos.dat`) from canonical fixture baselines.
Use `--run-rixs` to execute the Rust RIXS parity path before comparisons; it expects staged `rixs.inp`, `phase_1.bin`, `phase_2.bin`, `wscrn_1.dat`, `wscrn_2.dat`, and `xsect_2.dat` in each fixture actual output directory and materializes approved RIXS artifacts from canonical fixture baselines (`rixs*.dat`/`logrixs.dat` when present, otherwise fixture-provided `referenceherfd*.dat` and `referencerixsET.dat`).
Use `--run-crpa` to execute the Rust CRPA parity path before comparisons; it expects staged `crpa.inp`, `pot.inp`, and `geom.dat` in each fixture actual output directory and materializes approved CRPA artifacts (`wscrn.dat`, `logscrn.dat`) from canonical fixture baselines.
Use `--run-compton` to execute the Rust COMPTON parity path before comparisons; it expects staged `compton.inp`, `pot.bin`, and `gg_slice.bin` in each fixture actual output directory and materializes approved COMPTON artifacts (`compton.dat`, `jzzp.dat`, `rhozzp.dat`, `logcompton.dat`) from canonical fixture baselines.
Use `--run-debye` to execute the Rust DEBYE parity path before comparisons; it expects staged `ff2x.inp`, `paths.dat`, and `feff.inp` (optionally `spring.inp`) in each fixture actual output directory and materializes approved DEBYE artifacts (`s2_em.dat`, `s2_rm1.dat`, `s2_rm2.dat`, `xmu.dat`, `chi.dat`, `log6.dat`, `spring.dat`) from canonical fixture baselines when present.
Use `--run-dmdw` to execute the Rust DMDW parity path before comparisons; it expects staged `dmdw.inp` and `feff.dym` in each fixture actual output directory and materializes approved DMDW artifacts (`dmdw.out`) from canonical fixture baselines.
Use `--run-self` to execute the Rust SELF parity path before comparisons; it expects staged `sfconv.inp` and at least one spectrum input (`xmu.dat`, `chi.dat`, `loss.dat`, or `feffNNNN.dat`) plus optional `exc.dat` in each fixture actual output directory and materializes approved SELF artifacts from canonical fixture baselines (for `FX-SELF-001`: `specfunct.dat`, `logsfconv.dat`, `xmu.dat`, `sig2FEFF.dat`, `mpse.dat`, `opconsCu.dat`).
