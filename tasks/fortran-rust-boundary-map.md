# Fortran-to-Rust Boundary Map

This artifact defines the migration boundary contract between FEFF10 Fortran modules and Rust workspace targets. It is the implementation-sequencing reference for `US-013`.

## Workspace Boundary Contract

- `src/domain`: shared FEFF types, module identifiers, and compatibility error/result contracts used across parser, numerics, pipelines, and CLI.
- `src/parser`: FEFF input deck tokenization and AST construction, producing typed input structures for pipeline modules.
- `src/numerics`: deterministic numeric helper routines shared by scientific module ports.
- `src/pipelines`: module executors and orchestration contracts (`PipelineExecutor`), plus regression/comparator infrastructure.
- `src/cli`: command parsing and runtime wiring that selects pipeline execution paths.

## Module Mapping

| Fortran module | Rust target module | Fixture IDs (manifest) | Required upstream dependencies for safe porting |
| --- | --- | --- | --- |
| `RDINP` | `src/pipelines/rdinp.rs` (`PipelineModule::Rdinp`) | `FX-RDINP-001`, `FX-WORKFLOW-XAS-001` | Depends on `src/parser` and `src/domain` card/input contracts only; produces downstream deck artifacts (`pot.inp`, `paths.inp`, `fms.inp`, `xsph.inp`, etc.). |
| `POT` | `src/pipelines/pot.rs` (`PipelineModule::Pot`) | `FX-POT-001`, `FX-WORKFLOW-XAS-001` | Requires `RDINP` outputs (`pot.inp`, `geom.dat`) and shared numerics helpers. |
| `PATH` | `src/pipelines/path.rs` (`PipelineModule::Path`) | `FX-PATH-001`, `FX-WORKFLOW-XAS-001` | Requires `RDINP` outputs (`paths.inp`, `geom.dat`, `global.inp`) and `XSPH` output (`phase.bin`). |
| `FMS` | `src/pipelines/fms.rs` (`PipelineModule::Fms`) | `FX-FMS-001`, `FX-WORKFLOW-XAS-001` | Requires `RDINP` outputs (`fms.inp`, `geom.dat`, `global.inp`) and `XSPH` output (`phase.bin`). |
| `XSPH` | `src/pipelines/xsph.rs` (`PipelineModule::Xsph`) | `FX-XSPH-001`, `FX-WORKFLOW-XAS-001` | Requires `POT` output (`pot.bin`) plus `RDINP` outputs (`xsph.inp`, `geom.dat`, `global.inp`); may also consume `SCREEN`/`CRPA` output (`wscrn.dat`). |
| `BAND` | `src/pipelines/band.rs` (`PipelineModule::Band`) | `FX-BAND-001` | Requires `RDINP`-generated `band.inp` and `XSPH` output (`phase.bin`) with shared geometry/global artifacts. |
| `LDOS` | `src/pipelines/ldos.rs` (`PipelineModule::Ldos`) | `FX-LDOS-001` | Requires `RDINP` outputs (`ldos.inp`, `geom.dat`, `reciprocal.inp`) and `POT` output (`pot.bin`). |
| `RIXS` | `src/pipelines/rixs.rs` (`PipelineModule::Rixs`) | `FX-RIXS-001` | Requires multi-edge spectral inputs (`phase_1.bin`, `phase_2.bin`, `wscrn_1.dat`, `wscrn_2.dat`, `xsect_2.dat`) derived from repeated core-chain execution. |
| `CRPA` | `src/pipelines/crpa.rs` (`PipelineModule::Crpa`) | `FX-CRPA-001` | Requires `RDINP`/input artifacts (`crpa.inp`, `pot.inp`, `geom.dat`); produces screening artifact (`wscrn.dat`). |
| `COMPTON` | `src/pipelines/compton.rs` (`PipelineModule::Compton`) | `FX-COMPTON-001` | Requires `POT` output (`pot.bin`) and FMS-derived slices (`gg_slice.bin`) plus `compton.inp`. |
| `DEBYE` | `src/pipelines/debye.rs` (`PipelineModule::Debye`) | `FX-DEBYE-001` | Requires `PATH` output (`paths.dat`) and `RDINP` output (`ff2x.inp`) plus `feff.inp`/optional `spring.inp`. |
| `DMDW` | `src/pipelines/dmdw.rs` (`PipelineModule::Dmdw`) | `FX-DMDW-001` | Requires Debye workflow artifacts (`dmdw.inp`, `feff.dym`) and shared IO/error handling. |
| `SCREEN` | `src/pipelines/screen.rs` (`PipelineModule::Screen`) | `FX-SCREEN-001` | Requires `RDINP` outputs (`pot.inp`, `geom.dat`, `ldos.inp`, optional `screen.inp`) and may feed `XSPH` via `wscrn.dat`. |
| `SELF` | `src/pipelines/self_energy.rs` (`PipelineModule::SelfEnergy`) | `FX-SELF-001` | Requires `RDINP` output (`sfconv.inp`) and spectrum artifacts (`xmu.dat`, `chi.dat`, `feffNNNN.dat`) from prior modules. |
| `EELS` | `src/pipelines/eels.rs` (`PipelineModule::Eels`) | `FX-EELS-001` | Requires `RDINP` output (`eels.inp`) and spectral artifact (`xmu.dat`) from core spectroscopy chain. |
| `FULLSPECTRUM` | `src/pipelines/fullspectrum.rs` (`PipelineModule::FullSpectrum`) | `FX-FULLSPECTRUM-001` | Requires `RDINP` output (`fullspectrum.inp`) plus component spectra from `fms_re/`, `fms_im/`, `path_re/`, `path_im/`, and `fprime*/`. |

## Dependency-Ordered Implementation Sequence

The safe sequencing below follows compatibility-matrix artifact dependencies and preserves a serial-first execution path aligned with `D-2`.

1. Foundation contracts (already established): `src/domain`, `src/parser`, `src/numerics`, `src/pipelines` trait interfaces, and CLI wiring.
2. Core production chain:
   - `RDINP`
   - `POT`
   - `SCREEN` and `CRPA` (parallel after `POT`)
   - `XSPH`
   - `FMS` and `PATH` (parallel after `XSPH`)
3. Core-adjacent module ports:
   - `BAND` and `LDOS` (parallel after core chain artifacts are stable)
   - `COMPTON` (after `POT` and FMS artifact contract is stable)
   - `DEBYE`
   - `DMDW` (after `DEBYE`)
4. Spectroscopy/post-processing module ports:
   - `SELF`
   - `EELS`
   - `FULLSPECTRUM`
   - `RIXS` (after multi-edge core-chain artifact generation is stable)

## Traceability Inputs

- Compatibility contract source: `tasks/feff10-compatibility-matrix.md`
- Fixture manifest source: `tasks/golden-fixture-manifest.json`
- Current Rust boundary anchors: `src/domain/mod.rs`, `src/pipelines/mod.rs`
