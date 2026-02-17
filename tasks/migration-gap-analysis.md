# FEFF10-RS Migration Gap Analysis

## Context

The feff10-rs project aims to rewrite FEFF10 (Fortran X-ray spectroscopy code) into pure Rust with no Fortran runtime dependencies. All 16 core computational modules have Rust files with working input parsing, output format contracts, CLI dispatch, and regression infrastructure. However, **the actual physics algorithms have not been ported** -- every module's `model.rs` generates plausible-looking output using simple arithmetic formulas (sin/cos/exp) instead of real physics.

---

## What IS Done (Infrastructure & Scaffolding)

| Layer | Status | Key Files |
|-------|--------|-----------|
| Workspace architecture | Complete | `Cargo.toml` (workspace), `crates/feff-core/`, `crates/feff-cli/` |
| Domain model (16 modules, errors, artifacts) | Complete | `crates/feff-core/src/domain/mod.rs` |
| Input parsing (all 16 modules) | Complete | Each module's `parser.rs` (~7,700 lines total) |
| Output format contracts (magic headers, layouts) | Complete | Each module's `model.rs` (~5,800 lines total) |
| Module executor trait + dispatch | Complete | `modules/traits.rs`, `modules/dispatch.rs` |
| Regression/oracle comparison infrastructure | Complete | `modules/regression.rs`, `modules/comparator.rs` |
| CLI (clap, tracing, anyhow) | Complete | `crates/feff-cli/src/` |
| Numerics (tolerance checking, Kahan sum) | Complete | `crates/feff-core/src/numerics/mod.rs` |
| Golden fixture manifest (18 fixtures) | Complete | `tasks/golden-fixture-manifest.json` |
| CI workflows (quality + parity gates) | Complete | `.github/workflows/` |

---

## What IS NOT Done (Physics Computation)

### Evidence: Placeholder formulas in model.rs files

**POT** (`crates/feff-core/src/modules/pot/model.rs:277-283`):
```rust
// Placeholder: simple Z - ion
let vmt0 = -zeff / (radius_mean + 1.0) * (1.0 + 0.05 * index as f64) - local_density;
```
Real POT: Iterative SCF Hartree-Fock-Slater with muffin-tin approximations (~8,400 lines Fortran)

**XSPH** (`crates/feff-core/src/modules/xsph/model.rs:192-199`):
```rust
// Placeholder: sin oscillation with exponential damping
let phase = config.base_phase + config.phase_scale * oscillation * attenuation;
```
Real XSPH: Dirac equation solver for complex-energy photoelectron states (~9,600 lines Fortran)

**FMS** (`crates/feff-core/src/modules/fms/model.rs:190-201`):
```rust
// Placeholder: sin/cos with exponential envelope
let real = scattering * envelope * oscillation * radial_weight / channel_f.sqrt();
```
Real FMS: Full scattering matrix inversion via Green's function (~6,700 lines Fortran)

This pattern is consistent across **all 16 modules**.

---

## Unmigrated Fortran Physics Subsystems

### Tier 0: Mathematical Foundations

| Subsystem | Files | Key Algorithms | Used By |
|-----------|-------|----------------|---------|
| **MATH** (`feff10/src/MATH/`, 32 files) | `besjh.f90`, `ylm.f90`, `cwig3j.f90`, `lu.f90`, `invertmatrix.f90`, `seigen.f90`, `somm.f90`, `conv.f90` | Spherical Bessel Jl/Nl/Hl (complex arg), spherical harmonics Ylm, Wigner 3j/6j, LU decomposition, matrix inversion, eigenvalue solver, numerical integration, convolution | All modules |
| **EXCH** (`feff10/src/EXCH/`, 13 files) | `rhl.f90`, `xcpot.f90`, `edp.f90` | Exchange-correlation potentials (Hedin-Lundqvist, Von Barth-Hedin, Dirac-Hara, Perdew-Zunger) | POT, XSPH, SCREEN, SELF |
| **COMMON** (physics subset, 44 files) | `m_config.f90` (156KB), `getorb.f90`, `m_constants.f90`, `m_t3j.f90`, `isedge.f90`, `setgam.f90` | Electronic configuration database (all elements), orbital extraction, physical constants, angular momentum coupling, edge/lifetime determination | All modules |

### Tier 1: Atomic Physics

| Subsystem | Files | Key Algorithms | Used By |
|-----------|-------|----------------|---------|
| **ATOM** (`feff10/src/ATOM/`, 30 files) | `atomic.f90`, `soldir.f90`, `etotal.f90`, `s02at.f90`, `apot.f90` | Self-consistent Hartree-Fock-Slater, radial Dirac equation, total energy, S02 amplitude, muffin-tin potentials | POT |
| **FOVRG** (`feff10/src/FOVRG/`, 17 files) | `dfovrg.f90`, `potex.f90`, `solout.f90`, `solin.f90` | Complex-energy Dirac equation for photoelectron wavefunctions, exchange potential, muffin-tin boundary matching | XSPH, FMS |

### Tier 2: Scattering Physics

| Subsystem | Files | Key Algorithms | Used By |
|-----------|-------|----------------|---------|
| **GENFMT** (`feff10/src/GENFMT/`, 18 files) | `genfmt.f90`, `mmtr.f90`, `rot3i.f90` | Multiple scattering path expansion for EXAFS chi, scattering amplitude matrices, 3D rotation | PATH |
| **MKGTR** (`feff10/src/MKGTR/`, 5 files) | `mkgtr.f90`, `getgtr.f90`, `rotgmatrix.f90` | Full multiple scattering Green's function matrix inversion | FMS |

### Tier 3: Module-Specific Physics

| Subsystem | Lines | Consumed By |
|-----------|-------|-------------|
| **POT** Fortran (`feff10/src/POT/`, 34 files) | ~8,400 | POT module: SCF loop, Broyden mixing, Coulomb integrals |
| **XSPH** Fortran (`feff10/src/XSPH/`, 37 files) | ~9,600 | XSPH module: Phase shifts, cross sections |
| **FMS** Fortran (`feff10/src/FMS/`, 19 files) | ~6,700 | FMS module: Green's function matrix inversion |
| **BAND** Fortran (`feff10/src/BAND/`, 17 files) | ~16,400 | BAND module: KKR band structure |
| **LDOS** Fortran (`feff10/src/LDOS/`, 18 files) | ~4,900 | LDOS module: Density of states |
| **FF2X** (`feff10/src/FF2X/`, 20 files) | ~7,200 | DEBYE/EELS/FULLSPECTRUM: chi(k), mu(E), Debye-Waller |
| **SFCONV** (`feff10/src/SFCONV/`, 16 files) | ~5,900 | SELF module: Spectral function convolution |
| **TDLDA** (`feff10/src/TDLDA/`, 17 files) | ~4,200 | SCREEN/EELS: Dielectric response |
| **KSPACE** (`feff10/src/KSPACE/`, 38 files) | ~7,300 | BAND/LDOS: Brillouin zone integration |
| **RHORRP** (`feff10/src/RHORRP/`, 3 files) | ~500 | COMPTON: Electron momentum density |

### Numerics Module Gap

The Rust `numerics/mod.rs` currently contains **only** validation/comparison utilities:
- Tolerance checking, Kahan summation, linear interpolation, distance calculations
- **Missing**: Bessel functions, spherical harmonics, Wigner coefficients, matrix operations, ODE solvers, numerical integration, special functions

---

## Recommended Implementation Order

### Phase 1: Mathematical Kernel
1. **Core math primitives** -- Port MATH/ into `numerics/` (Bessel, Ylm, Wigner 3j, linear algebra, integration, interpolation)
2. **Exchange-correlation** -- Port EXCH/ into `numerics/exchange`
3. **Constants & config database** -- Port physics-critical COMMON/ (m_config.f90, getorb.f90, m_constants.f90)

### Phase 2: Atomic Solver
4. **Atomic SCF** -- Port ATOM/ (Hartree-Fock-Slater, radial Dirac equation, muffin-tin)
5. **Complex-energy Dirac solver** -- Port FOVRG/ (photoelectron wavefunctions)

### Phase 3: Core Chain (replace placeholder model.rs files)
6. **POT** -- Real SCF solver (first module for oracle validation)
7. **SCREEN + CRPA** (parallel, both consume POT output)
8. **XSPH** -- Real Dirac equation phase shifts
9. **FMS + PATH** (parallel: MKGTR matrix inversion + GENFMT path expansion)

### Phase 4: Remaining Modules
10. **BAND + LDOS** (parallel, port KSPACE/)
11. **DEBYE + DMDW + COMPTON** (parallel, port FF2X/, RHORRP/)
12. **SELF + EELS + FULLSPECTRUM + RIXS** (port SFCONV/, TDLDA/)

---

## Useful Rust Crates

| Crate | Purpose |
|-------|---------|
| `num-complex` | Complex number arithmetic (essential -- virtually all FEFF physics uses complex numbers) |
| `nalgebra` or `faer` | Dense matrix operations (LU, inverse, eigenvalues) for FMS/GENFMT |
| `ndarray` | N-dimensional arrays for radial grids, angular momentum arrays |
| Custom port | Complex-argument spherical Bessel functions (no crate handles FEFF's branch cuts) |

---

## Verification

Each phase can be validated incrementally using the existing infrastructure:
1. `cargo test` -- Unit tests for new math primitives against known reference values
2. `cargo run -- oracle` with `golden-fixture-manifest.json` and `numeric-tolerance-policy.json` -- Compare Rust output against Fortran reference for each module as physics is ported
3. Parity gate CI (`.github/workflows/rust-parity-gates.yml`) -- Automated regression checking
