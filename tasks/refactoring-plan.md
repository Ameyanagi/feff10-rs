# Refactoring Plan: feff10-rs Directory Structure

## Context

The project is a single-crate Rust rewrite of the FEFF10 Fortran physics code. Nearly all computation code lives in `src/pipelines/` (~750KB across 20 files), with individual files ranging 24-99KB. The name "pipelines" is misleading — these are computational modules (POT, XSPH, PATH, etc.). The goal is better naming, navigability, and a clean lib+bin workspace split.

## Current Structure (the problem)

```
src/
├── lib.rs / main.rs          (tiny entry points)
├── cli/mod.rs                (42KB monolith — CLI parsing + 5 command handlers)
├── domain/mod.rs + errors.rs (16KB — core types all named "Pipeline*")
├── numerics/mod.rs           (12KB — math utilities)
├── parser/mod.rs             (16KB — input deck tokenizer)
└── pipelines/                (~750KB — misleading name)
    ├── mod.rs                (52KB — traits + dispatch + 16 executor wrappers + helpers)
    ├── regression.rs         (99KB) + comparator.rs (39KB)
    ├── serialization.rs      (3KB)
    └── 16 computation files  (24-51KB each)
```

---

## Step 0: Rename types and directory (in-place, before workspace split)

Rename `Pipeline*` types throughout the codebase:

| Current | Proposed |
|---|---|
| `pipelines/` directory | `modules/` |
| `PipelineModule` | `ComputeModule` |
| `PipelineRequest` | `ComputeRequest` |
| `PipelineArtifact` | `ComputeArtifact` |
| `PipelineResult<T>` | `ComputeResult<T>` |
| `PipelineExecutor` | `ModuleExecutor` |
| `RuntimePipelineExecutor` | `RuntimeModuleExecutor` |
| `ValidationPipelineExecutor` | `ValidationModuleExecutor` |
| `XxxPipelineScaffold` | `XxxModule` (e.g. `BandModule`) |
| `XxxPipelineInterface` | `XxxContract` |
| `execute_runtime_pipeline` | `execute_runtime_module` |

**Files touched**: all `src/` files + all `tests/` files (mechanical find-and-replace).

## Step 1: Split each computation file into sub-directories

Convert each `modules/band.rs` (37KB) into:

```
modules/band/
  mod.rs       — BandModule, BandContract, ModuleExecutor impl, constants, tests
  model.rs     — BandModel, BandControlInput, GeomBandInput, etc.
  parser.rs    — parse_band_source(), parse_geom_source(), etc.
  output.rs    — render_bandstructure(), render_logband(), write_artifact()
```

Repeat for all 16 modules. Start with the smallest (`dmdw` at 25KB) as a template, then do the rest.

## Step 2: Split `modules/mod.rs` (52KB)

Break into:
- `modules/traits.rs` — `ModuleExecutor`, `RuntimeModuleExecutor`, `ValidationModuleExecutor`
- `modules/dispatch.rs` — `execute_runtime_module()`, `runtime_compute_engine_available()`, 16 `RuntimeXxxExecutor` wrappers
- `modules/helpers.rs` — `CoreModuleHelper`, `DistanceShell`, utility functions

`mod.rs` becomes a thin re-export file.

## Step 3: Split `cli/mod.rs` (42KB)

Break into:
- `cli/mod.rs` — `run_from_env()`, `run()`, `CliError`, dispatch skeleton (~100 lines)
- `cli/dispatch.rs` — `ModuleCommandSpec`, module dispatch logic
- `cli/manifest.rs` — `CliManifest`, `CliManifestFixture`, loading
- `cli/regression_cmd.rs` — regression command handler
- `cli/oracle_cmd.rs` — oracle command handler
- `cli/feff_cmd.rs` — feff/feffmpi command handlers
- `cli/usage.rs` — usage text functions
- `cli/helpers.rs` — `find_workspace_root()`, `resolve_cli_path()`, etc.

## Step 4: Split into 2-crate workspace (core + cli)

Two crates only — `feff-core` (library) and `feff-cli` (binary):

```
feff10-rs/
├── Cargo.toml                    (workspace definition)
├── crates/
│   ├── feff-core/                (library crate — everything except CLI)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            (re-exports all sub-modules)
│   │       ├── domain/
│   │       │   ├── mod.rs        (ComputeModule, ComputeRequest, ComputeArtifact, InputDeck, etc.)
│   │       │   └── errors.rs     (FeffError, FeffErrorCategory)
│   │       ├── numerics/
│   │       │   └── mod.rs        (math utilities)
│   │       ├── parser/
│   │       │   └── mod.rs        (input deck tokenizer/parser)
│   │       └── modules/          (16 compute modules + regression/comparator)
│   │           ├── mod.rs        (thin re-exports)
│   │           ├── traits.rs     (ModuleExecutor, RuntimeModuleExecutor, etc.)
│   │           ├── dispatch.rs   (execute_runtime_module, 16 RuntimeXxxExecutor wrappers)
│   │           ├── helpers.rs    (CoreModuleHelper, DistanceShell)
│   │           ├── serialization.rs
│   │           ├── comparator.rs
│   │           ├── regression.rs
│   │           ├── band/         (mod.rs, model.rs, parser.rs, output.rs)
│   │           ├── compton/      ...
│   │           ├── crpa/         ...
│   │           ├── debye/        ...
│   │           ├── dmdw/         ...
│   │           ├── eels/         ...
│   │           ├── fms/          ...
│   │           ├── fullspectrum/ ...
│   │           ├── ldos/         ...
│   │           ├── path/         ...
│   │           ├── pot/          ...
│   │           ├── rdinp/        ...
│   │           ├── rixs/         ...
│   │           ├── screen/       ...
│   │           ├── self_energy/  ...
│   │           └── xsph/         ...
│   │
│   └── feff-cli/                 (binary crate — CLI only)
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           └── cli/
│               ├── mod.rs            (run_from_env, run, CliError, dispatch skeleton)
│               ├── dispatch.rs       (ModuleCommandSpec, module dispatch)
│               ├── manifest.rs       (CliManifest, fixture loading)
│               ├── regression_cmd.rs
│               ├── oracle_cmd.rs
│               ├── feff_cmd.rs
│               ├── usage.rs
│               └── helpers.rs
│
└── tests/                        (CLI integration tests stay at workspace root)
```

### Workspace `Cargo.toml`

```toml
[workspace]
resolver = "3"
members = ["crates/feff-core", "crates/feff-cli"]

[workspace.package]
version = "0.1.0"
edition = "2024"

[workspace.dependencies]
feff-core = { path = "crates/feff-core" }
globset = "0.4"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tempfile = "3.13"
```

### Dependency graph

```
feff-core  (library: globset, serde, serde_json)
  ↓
feff-cli   (binary: feff-core, serde, serde_json)
```

### What goes where

| Content | Crate |
|---|---|
| Domain types (`ComputeModule`, `ComputeRequest`, errors) | `feff-core` |
| Numerics (math utilities, tolerance) | `feff-core` |
| Parser (input deck tokenizer) | `feff-core` |
| 16 compute modules (band, pot, xsph, ...) | `feff-core` |
| Regression engine + comparator | `feff-core` |
| Serialization helpers | `feff-core` |
| CLI parsing, dispatch, commands | `feff-cli` |
| `main()` entry point | `feff-cli` |

### Publishing

- `cargo publish -p feff-core` — library for programmatic use
- `cargo publish -p feff-cli` — installs the `feff10-rs` binary
- Only 2 versions to manage

## Step 5: Cleanup

- De-duplicate binary magic constants (ldos/compton re-declare POT/FMS constants locally — import from source modules instead)
- Minimize `pub` exports
- Move module-specific parity tests (`*_parity.rs`) to `crates/feff-core/tests/`
- Keep CLI-level integration tests (`cli_compatibility.rs`, `regression_cli.rs`) at workspace root
- Set `[[bin]] name = "feff10-rs"` in `feff-cli/Cargo.toml` to preserve binary name

## Key considerations

- **Cross-module deps**: `band`, `fms`, `path` import `xsph::XSPH_PHASE_BINARY_MAGIC` — all stay within `feff-core::modules`, no issue
- **rdinp → parser**: Only compute module that crosses into parser. Both stay in `feff-core`, so import path is `crate::parser::parse_input_deck`
- **Integration tests**: `cli_compatibility.rs` invokes the binary via `Command::new` — stays at workspace root
- **Binary name**: `[[bin]] name = "feff10-rs"` in `feff-cli/Cargo.toml` preserves the existing binary name

---

## Ecosystem Crate Opportunities

### HIGH priority — `clap` (CLI parsing)

The 42KB `cli/mod.rs` has **200+ lines of hand-rolled argument parsing**: manual `args.remove(0)`, string-matching dispatch, `value_for_option()` helpers, and 5 hardcoded `*_usage_text()` functions. `clap` with derive macros would:
- Eliminate all usage text functions (auto-generated help)
- Replace manual option parsing with typed derive structs
- Add shell completions for free
- Estimated savings: 150-200 lines

### MEDIUM priority — `thiserror` (error boilerplate)

Three error types have hand-written `Display` + `Error` impls:
- `NumericTolerancePolicyError` in `src/numerics/mod.rs` (2 variants)
- `ComparatorError` in `src/pipelines/comparator.rs` (12 variants)
- `NumericParseError` in `src/numerics/mod.rs`

`thiserror` derive would replace ~80 lines of boilerplate. Note: `FeffError` itself should stay custom (exit code mapping logic is domain-specific).

### LOW priority — `tracing` (structured logging)

Currently only `println!`/`eprintln!`. Not urgent, but `tracing` would provide:
- Log levels and filtering for internal diagnostics
- Structured spans for pipeline execution timing
- No change to user-facing CLI output

### Keep as-is (no crate needed)

| Area | Why keep custom |
|---|---|
| Float comparison (`numerics/`) | Domain-specific tolerance policy with `relative_floor` — `approx` crate is too generic |
| Text diffing (`comparator.rs`) | Only needs "where's the mismatch?", not unified diffs — `similar` would be overkill |
| File paths (`std::path`) | No UTF-8 issues; `camino` adds dependency for no real gain |
| Snapshot tests | Working manual system; `insta` is nice-to-have but not urgent |
| Parallelism | Serial-only by design; `rayon` is future work when perf matters |

---

## Verification

After each step:
1. `cargo build` compiles cleanly
2. `cargo test` passes all existing tests
3. `cargo clippy` has no new warnings
