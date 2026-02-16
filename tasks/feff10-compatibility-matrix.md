# FEFF10 Compatibility Matrix

This matrix is the canonical compatibility contract for FEFF10 migration work. It defines module-level CLI surfaces, required artifacts, output locations, and fixture traceability.

## Top-Level CLI Contract

| Command | Required options or arguments | Contracted behavior | Output directory |
| --- | --- | --- | --- |
| `feff` | None. Run in a directory that contains `feff.inp`. | Runs the serial FEFF module chain in migration order. | Current working directory (`.`). |
| `feffmpi <nprocs>` | `<nprocs>` integer process count. | Runs the MPI wrapper chain with the same module ordering contract. | Current working directory (`.`). |
| `<module executable>` | None for direct module invocations (`rdinp`, `pot`, `xsph`, etc.). | Runs one module against previously generated intermediate artifacts. | Current working directory (`.`). |

## Module Compatibility Matrix

| Module | CLI command surface | Required options or cards | Required input artifacts | Required output artifacts | Output directory contract | Fixture IDs |
| --- | --- | --- | --- | --- | --- | --- |
| `RDINP` | `feff`, `rdinp` | `feff.inp` required; module gating through `CONTROL` card values. | `feff.inp` | `geom.dat`, `global.inp`, `reciprocal.inp`, `pot.inp`, `ldos.inp`, `screen.inp` (when `SCREEN` card is present), `xsph.inp`, `fms.inp`, `paths.inp`, `genfmt.inp`, `ff2x.inp`, `sfconv.inp`, `eels.inp`, `compton.inp`, `band.inp`, `rixs.inp`, `crpa.inp`, `fullspectrum.inp`, `dmdw.inp`, `log.dat` | `.` | `FX-RDINP-001`, `FX-WORKFLOW-XAS-001` |
| `POT` | `feff`, `pot` | `SCF` and `POTENTIALS` cards define potential workflow behavior. | `pot.inp`, `geom.dat` | `pot.bin`, `pot.dat`, `log1.dat`, `convergence.scf`, `convergence.scf.fine` | `.` | `FX-POT-001`, `FX-WORKFLOW-XAS-001` |
| `PATH` | `feff`, `path` | `RPATH` required for path enumeration; `NLEG` and path filters optional. | `paths.inp`, `geom.dat`, `global.inp`, `phase.bin` | `paths.dat`, `paths.bin`, `crit.dat`, `log4.dat` | `.` | `FX-PATH-001`, `FX-WORKFLOW-XAS-001` |
| `FMS` | `feff`, `fms` | `FMS` card controls cluster radius and enablement. | `fms.inp`, `geom.dat`, `global.inp`, `phase.bin` | `gg.bin` (historically consumed as FMS binary output), `log3.dat` | `.` | `FX-FMS-001`, `FX-WORKFLOW-XAS-001` |
| `XSPH` | `feff`, `xsph` | `XSPH` stage active via `xsph.inp`; advanced behavior via cards like `LJMAX`, `LDEC`, `RSIGMA`, `TDLDA`. | `xsph.inp`, `geom.dat`, `global.inp`, `pot.bin`, `wscrn.dat` (if screened core hole is enabled) | `phase.bin`, `xsect.dat`, `log2.dat`, `phase.dat` (formatted optional output) | `.` | `FX-XSPH-001`, `FX-WORKFLOW-XAS-001` |
| `BAND` | `band` | `BANDSTRUCTURE` card required to populate `band.inp` contract values. | `band.inp`, `geom.dat`, `global.inp`, `phase.bin` | `bandstructure.dat`, `logband.dat` | `.` | `FX-BAND-001` |
| `LDOS` | `feff`, `ldos` | `LDOS` card required for DOS range and step configuration. | `ldos.inp`, `geom.dat`, `pot.bin`, `reciprocal.inp` | `ldos00.dat` and `ldosNN.dat` series, `logdos.dat` | `.` | `FX-LDOS-001` |
| `RIXS` | `rixs` | `RIXS` card and `rixs.inp` edge settings required. | `rixs.inp`, `phase_1.bin`, `phase_2.bin`, `wscrn_1.dat`, `wscrn_2.dat`, `xsect_2.dat` | `rixs0.dat`, `rixs1.dat`, `rixsET.dat`, `rixsEE.dat`, `rixsET-sat.dat`, `rixsEE-sat.dat`, `logrixs.dat` | `.` | `FX-RIXS-001` |
| `CRPA` | `crpa` | CRPA must be enabled (`do_CRPA = 1` in `crpa.inp`). | `crpa.inp`, `pot.inp`, `geom.dat` | `wscrn.dat`, `logscrn.dat` | `.` | `FX-CRPA-001` |
| `COMPTON` | `feff`, `compton` | `COMPTON` card required; `CGRID` and `RHOZZP` optional for grid and density output controls. | `compton.inp`, `pot.bin`, `gg_slice.bin` | `compton.dat`, `jzzp.dat`, `rhozzp.dat`, `logcompton.dat` | `.` | `FX-COMPTON-001` |
| `DEBYE` | `feff`, `ff2x` | `DEBYE` card in `feff.inp` enables Debye-Waller path workflow. | `ff2x.inp`, `paths.dat`, `feff.inp`, `spring.inp` (if provided) | `s2_em.dat`, `s2_rm1.dat`, `s2_rm2.dat`, updated spectrum outputs (`xmu.dat` and/or `chi.dat`), `log6.dat` | `.` | `FX-DEBYE-001` |
| `DMDW` | `feff`, `dmdw` | DMDW route enabled through `dmdw.inp` content generated from `DEBYE ... DW_Opt` inputs. | `dmdw.inp`, `feff.dym` | `dmdw.out` | `.` | `FX-DMDW-001` |
| `SCREEN` | `feff`, `screen` | Screened-core-hole path requires `COREHOLE RPA`; optional overrides from `SCREEN` card. | `pot.inp`, `geom.dat`, `ldos.inp`, `screen.inp` (optional override file) | `wscrn.dat`, `logscreen.dat` | `.` | `FX-SCREEN-001` |
| `SELF` | `feff`, `sfconv` | `SELF` and/or `SFSE` cards in `sfconv.inp`; no standalone `self` executable is part of the module program list. | `sfconv.inp`, one or more spectra (`xmu.dat`, `chi.dat`, `feffNNNN.dat`), `exc.dat` (optional) | `selfenergy.dat`, `sigma.dat`, `specfunct.dat`, rewritten input spectrum files, `logsfconv.dat` | `.` | `FX-SELF-001` |
| `EELS` | `feff`, `eels` | `ELNES` or `EXELFS` card required; `MAGIC` optional. | `eels.inp`, `xmu.dat` | `eels.dat`, `magic.dat` (when requested), `logeels.dat` | `.` | `FX-EELS-001` |
| `FULLSPECTRUM` | `fullspectrum` | `fullspectrum.inp` required for combined-spectrum run control. | `fullspectrum.inp`, component `xmu.dat` paths from `fms_re/`, `fms_im/`, `path_re/`, `path_im/`, `fprime*/` (as configured) | `xmu.dat`, `osc_str.dat`, `eps.dat`, `drude.dat`, `background.dat`, `fine_st.dat`, `logfullspectrum.dat` | Primary outputs in `.`; component inputs may reside in configured subdirectories. | `FX-FULLSPECTRUM-001` |

## Fixture ID Convention

- Module fixtures use `FX-<MODULE>-001` identifiers.
- Shared pipeline fixtures use `FX-WORKFLOW-<NAME>-NNN`.
- These IDs are reserved and must be reused verbatim in the fixture manifest story (`US-006`).
