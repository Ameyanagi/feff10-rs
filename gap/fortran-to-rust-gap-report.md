# Fortran-to-Rust Gap Report

Generated: 2026-02-19 13:59:11 JST

## Scope
- Source scanned: `feff10/src/**/*.f*`
- Runtime-module contract source: `tasks/golden-fixture-manifest.json` (`inScopeModules`)
- Full file inventory: `gap/fortran-to-rust-file-gap.csv`
- Migration closeout overlay source: `tasks/full-module-migration-ledger.csv`
- Unresolved policy: conservative external (no expansion through unresolved targets)

## Method
- Parse each Fortran file for `module`, `subroutine`, `function` definitions.
- Parse `use` and `call` references and resolve to local definitions when possible.
- Seed graph traversal with runtime-owned files from v1 module directories.
- Apply closeout overlay: when ledger rows are all `status=done` and `rust_target` files exist, classify all remaining non-runtime-owned Fortran files as `migrated_to_rust`.
- Classify files as `runtime_owned`, `migrated_to_rust`, `runtime_support_dependency`, or `out_of_scope`.

## Totals
- Total Fortran source files found: **527**
- `runtime_owned`: **272**
- `migrated_to_rust`: **255**
- `runtime_support_dependency`: **0**
- `out_of_scope`: **0**
- `remaining_gap`: **0**

## Closeout Overlay
- Ledger rows: **102**
- Ledger rows migrated (`status=done` + existing `rust_target`): **102**
- Ledger rows pending/non-done: **0**
- Ledger rows with missing `rust_target`: **0**
- Full-module overlay active: **yes**

## Directory Coverage
| Fortran Dir | File Count | runtime_owned | migrated_to_rust | runtime_support_dependency | out_of_scope |
| --- | ---: | ---: | ---: | ---: | ---: |
| ATOM | 30 | 0 | 30 | 0 | 0 |
| BAND | 15 | 15 | 0 | 0 | 0 |
| COMMON | 44 | 0 | 44 | 0 | 0 |
| COMPTON | 3 | 3 | 0 | 0 | 0 |
| CRPA | 2 | 2 | 0 | 0 | 0 |
| DEBYE | 5 | 5 | 0 | 0 | 0 |
| DMDW | 6 | 6 | 0 | 0 | 0 |
| DRIVERS | 9 | 0 | 9 | 0 | 0 |
| EELS | 16 | 16 | 0 | 0 | 0 |
| EELSMDFF | 12 | 0 | 12 | 0 | 0 |
| ERRORMODS | 3 | 0 | 3 | 0 | 0 |
| EXCH | 12 | 0 | 12 | 0 | 0 |
| FF2X | 19 | 19 | 0 | 0 | 0 |
| FMS | 19 | 19 | 0 | 0 | 0 |
| FOVRG | 17 | 0 | 17 | 0 | 0 |
| FULLSPECTRUM | 20 | 20 | 0 | 0 | 0 |
| GENFMT | 18 | 0 | 18 | 0 | 0 |
| HEADERS | 1 | 0 | 1 | 0 | 0 |
| INPGEN | 4 | 0 | 4 | 0 | 0 |
| IOMODS | 3 | 0 | 3 | 0 | 0 |
| KSPACE | 40 | 0 | 40 | 0 | 0 |
| LDOS | 18 | 18 | 0 | 0 | 0 |
| MATH | 32 | 0 | 32 | 0 | 0 |
| MKGTR | 5 | 0 | 5 | 0 | 0 |
| OPCONSAT | 4 | 0 | 4 | 0 | 0 |
| PAR | 1 | 0 | 1 | 0 | 0 |
| PATH | 17 | 17 | 0 | 0 | 0 |
| POT | 34 | 34 | 0 | 0 | 0 |
| RDINP | 18 | 18 | 0 | 0 | 0 |
| RHORRP | 3 | 0 | 3 | 0 | 0 |
| RIXS | 6 | 6 | 0 | 0 | 0 |
| SCREEN | 8 | 8 | 0 | 0 | 0 |
| SELF | 15 | 15 | 0 | 0 | 0 |
| SFCONV | 14 | 14 | 0 | 0 | 0 |
| TDLDA | 16 | 0 | 16 | 0 | 0 |
| UTILITY | 1 | 0 | 1 | 0 | 0 |
| XSPH | 37 | 37 | 0 | 0 | 0 |

## Top Runtime Support Directories
No runtime support dependency files were detected outside runtime-owned directories.

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
| `call::par_recv_dc` | 3 |
| `call::par_send_dc` | 3 |
| `call::par_send_double` | 2 |
| `call::par_recv_double` | 2 |
| `call::par_bcast_int` | 2 |
| `call::par_bcast_double` | 2 |
| `use::bp` | 2 |

## Notes
- This is static parsing, not a full Fortran semantic/AST analysis.
- `call`/`use` symbols may resolve to multiple files; reachability uses unioned edges.
- `feff10/` is a local reference checkout (not tracked in git).
