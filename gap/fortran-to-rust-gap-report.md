# Fortran-to-Rust Gap Report

Generated: 2026-02-18 16:26:36 JST

## Scope
- Source scanned: `feff10/src/**/*.f*`
- Runtime-module contract source: `tasks/golden-fixture-manifest.json` (`inScopeModules`)
- Full file inventory: `gap/fortran-to-rust-file-gap.csv`
- Unresolved policy: conservative external (no expansion through unresolved targets)

## Method
- Parse each Fortran file for `module`, `subroutine`, `function` definitions.
- Parse `use` and `call` references and resolve to local definitions when possible.
- Seed graph traversal with runtime-owned files from v1 module directories.
- Classify files as `runtime_owned`, `runtime_support_dependency`, or `out_of_scope`.

## Totals
- Total Fortran source files found: **527**
- `runtime_owned`: **272**
- `runtime_support_dependency`: **153**
- `out_of_scope`: **102**

## Directory Coverage
| Fortran Dir | File Count | runtime_owned | runtime_support_dependency | out_of_scope |
| --- | ---: | ---: | ---: | ---: |
| ATOM | 30 | 0 | 1 | 29 |
| BAND | 15 | 15 | 0 | 0 |
| COMMON | 44 | 0 | 38 | 6 |
| COMPTON | 3 | 3 | 0 | 0 |
| CRPA | 2 | 2 | 0 | 0 |
| DEBYE | 5 | 5 | 0 | 0 |
| DMDW | 6 | 6 | 0 | 0 |
| DRIVERS | 9 | 0 | 0 | 9 |
| EELS | 16 | 16 | 0 | 0 |
| EELSMDFF | 12 | 0 | 3 | 9 |
| ERRORMODS | 3 | 0 | 2 | 1 |
| EXCH | 12 | 0 | 11 | 1 |
| FF2X | 19 | 19 | 0 | 0 |
| FMS | 19 | 19 | 0 | 0 |
| FOVRG | 17 | 0 | 13 | 4 |
| FULLSPECTRUM | 20 | 20 | 0 | 0 |
| GENFMT | 18 | 0 | 0 | 18 |
| HEADERS | 1 | 0 | 0 | 1 |
| INPGEN | 4 | 0 | 2 | 2 |
| IOMODS | 3 | 0 | 3 | 0 |
| KSPACE | 40 | 0 | 37 | 3 |
| LDOS | 18 | 18 | 0 | 0 |
| MATH | 32 | 0 | 25 | 7 |
| MKGTR | 5 | 0 | 0 | 5 |
| OPCONSAT | 4 | 0 | 0 | 4 |
| PAR | 1 | 0 | 1 | 0 |
| PATH | 17 | 17 | 0 | 0 |
| POT | 34 | 34 | 0 | 0 |
| RDINP | 18 | 18 | 0 | 0 |
| RHORRP | 3 | 0 | 1 | 2 |
| RIXS | 6 | 6 | 0 | 0 |
| SCREEN | 8 | 8 | 0 | 0 |
| SELF | 15 | 15 | 0 | 0 |
| SFCONV | 14 | 14 | 0 | 0 |
| TDLDA | 16 | 0 | 15 | 1 |
| UTILITY | 1 | 0 | 1 | 0 |
| XSPH | 37 | 37 | 0 | 0 |

## Top Runtime Support Directories
| Fortran Dir | runtime_support_dependency files |
| --- | ---: |
| COMMON | 38 |
| KSPACE | 37 |
| MATH | 25 |
| TDLDA | 15 |
| FOVRG | 13 |
| EXCH | 11 |
| EELSMDFF | 3 |
| IOMODS | 3 |
| ERRORMODS | 2 |
| INPGEN | 2 |
| ATOM | 1 |
| PAR | 1 |
| RHORRP | 1 |
| UTILITY | 1 |

## Unresolved Target Diagnostics
- Files with unresolved targets: **173**
- Total unresolved target entries (unique per file): **585**
- Top unresolved targets by file count:

| Target | Files |
| --- | ---: |
| `call::par_stop` | 75 |
| `call::par_barrier` | 25 |
| `call::par_end` | 22 |
| `call::par_begin` | 22 |
| `call::seconds` | 14 |
| `call::error` | 10 |
| `call::write2d` | 6 |
| `call::par_send_cmplx` | 6 |
| `call::par_recv_cmplx` | 6 |
| `call::read2d` | 5 |
| `call::par_recv_int_scalar` | 5 |
| `call::par_send_int_scalar` | 5 |
| `use::tfrm` | 3 |
| `call::par_send_dc` | 3 |
| `call::par_recv_dc` | 3 |
| `call::par_recv_double` | 2 |
| `call::par_send_double` | 2 |
| `call::par_bcast_double` | 2 |
| `call::par_bcast_int` | 2 |
| `use::bp` | 2 |

## Notes
- This is static parsing, not a full Fortran semantic/AST analysis.
- `call`/`use` symbols may resolve to multiple files; reachability uses unioned edges.
- `feff10/` is a local reference checkout (not tracked in git).
