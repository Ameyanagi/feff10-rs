# Codebase review — 2026-09-06

Scope: the Rust library, FFI/build system, CLI, Python bindings, tests, documentation,
and CI/release workflows, with a focused trace through the Fortran calculation and
I/O routines relevant to issue #1. This is an architecture and implementation review,
not a line-by-line audit of every upstream scientific routine. The product currently
has CLI and library interfaces; UI recommendations below concern those interfaces.

## Issue #1: findings and changes

- **Windows:** `Auto` previously fell back to in-process execution without the worker
  hook. `pot`, `xsph`, and other stages allocate shared `lrstat` state. A later stage
  can abort on a second allocation. The CLI also omitted `worker::init()` entirely.
  The CLI now installs the hook, workers receive a 64 MiB stack, and unsupported
  isolation configurations return errors before writing input files. Rust embedding
  applications on Windows must call `feff10::worker::init()` at the start of `main()`.
- **Linux:** reproduced the published Intel v0.2.2 archive's failure in Linux x86_64
  emulation using the bundled Cu input with `RPATH 5.2` and `PRINT 0 0 0 0 0 3`.
  PATH enumerates 15 paths, but GENFMT retains none. Verbose GENFMT output reveals
  `NaN` importance ratios. Probes identify zero termination matrices from `mmtr`,
  despite valid `bcoef`, radial matrix-element, and rotation-matrix inputs. GNU
  GENFMT works on the same intermediate files. Fresh processes and explicit Intel
  runtime initialization do not resolve the failure. Recompiling **only `mmtr`**
  with Intel 2024.0.1 at `-O1` restores the GNU amplitudes. The build now applies
  `-O1` specifically to this routine when using ifx, preserving the scientific code
  and optimization settings elsewhere.
- **False success:** the build previously changed fatal `par_stop` calls into
  returns. These now stop the stage with status 1. The wrapper preserves the FEFF
  error file on failed stages and reports `Finished` only after error checks.
- **Working-directory state:** the execution lock now covers input/error-file
  preparation; Unix children change their own cwd instead of changing the host's.
- **Regression coverage:** the worker-host test checks both Worker and Auto modes,
  repeated calculations, recovery after a fatal stage error, 14 Cu path files,
  finite/nonzero amplitudes, a nonzero EXAFS spectrum, and first-path geometry
  (`nleg=2`, `degen=12`, `reff=2.5527 Å`). Release jobs now test their actual archive
  through the prebuilt feature. Windows gains real calculation smoke tests.
- Two parser parity fixtures were invalid but passed when fatal stops were disabled:
  the overlap fixture lacked the absorber's overlap shell, and the periodic lattice
  fixture used potential 0 for every replicated site. Both now use valid inputs.

The released v0.2.2 assets are unchanged. The ifx fix and fatal-stop change require
rebuilding and publishing the native archives in a subsequent release. This branch
has not published a release or closed the issue. Windows runtime validation must
complete on the newly configured Windows CI jobs.

## Recommended next work

| Priority | Area | Finding and evidence | Proposed improvement |
|---|---|---|---|
| High | Calculation workflow | [`Stage::control_index`](../crates/feff10/src/stage.rs) maps POT to flag 1, XSPH to 2, and FMS to 3 (zero-based). [`rdinp.f90`](../feff10/src/RDINP/rdinp.f90) reads POT/XSPH/FMS from flags 0/1/2. The all-ones regression cannot expose this. | Correct the scheduling map and give RDINP its own rule. Test incremental CONTROL configurations against native FEFF, including reuse of existing potentials. |
| High | Python execution | [`config.rs`](../crates/feff10-python/src/config.rs) and [`lib.rs`](../crates/feff10-python/src/lib.rs) do not expose a worker entry point or isolation choice. Python cannot install a Rust hook in the interpreter's `main`. | Add a Python-aware subprocess worker (`python -m ...`) or a packaged helper executable. Run installed-wheel calculations on Windows and macOS GUI/notebook hosts before publishing wheels. The current Rust guard prevents the previous Windows host abort but does not provide that Python execution route. |
| High | Output correctness | [`FeffTable`](../crates/feff10/src/output.rs) accepts ragged rows in permissive mode, creating columns of different lengths; `r_squared`/`interp` assume matching lengths and can index out of bounds. Strict mode also accepts `NaN`/infinity. | Validate comparison inputs, finite values, increasing grids, and column lengths; return a structured error for invalid spectra. Give permissive parsing explicit missing-value semantics. |
| High | Release reliability | [`publish.yml`](../.github/workflows/publish.yml) ends each retry loop with `sleep`; exhausting all failed publish attempts can still give the shell a successful status. | Track publish success and return nonzero after the last failure. Validate all package versions and package contents before publishing. |
| High | Input/work-directory UX | [`cmd_run` and `cmd_bench`](../crates/feff10-cli/src/main.rs) stage only `feff.inp`. Benchmarking parses the copied file, so relative INCLUDE/LOAD paths resolve in the wrong directory. CIF and Debye matrix files are also needed by some calculations. | Parse includes from the original location and provide an explicit way to stage referenced auxiliary files. Record the resolved input and dependencies in the run directory. |
| Medium | CLI output | [`cmd_run`](../crates/feff10-cli/src/main.rs) hides the progress bar for `--json`/`--quiet`, while native stage output inherits stdout/stderr. Whether JSON stays clean depends on the compiler/runtime's buffering. | Route native output to per-stage logs and reserve stdout for one JSON document. Add fields for isolation, output paths, warnings, and log paths; make `--quiet` consistent. |
| Medium | Run safety and diagnostics | [`FeffPipeline`](../crates/feff10/src/pipeline.rs) writes `feff.inp` into the chosen directory and leaves prior outputs present. Discovery cannot tell fresh results from stale files; other processes can write the same directory. | Add run manifests, explicit fresh-run/resume behavior, and a directory lock. Report skipped versus executed stages and requested output products. Support cancellation and capture bounded stderr with stage failures. |
| Medium | Benchmark UX | [`cmd_bench`](../crates/feff10-cli/src/main.rs) returns success even if all inputs are missing or all iterations fail; nonfinite timing values become poor machine-readable results. The missing-input success case was reproduced locally. | Return nonzero for failed benchmarks, expose attempted/successful/failed counts, and retain failure diagnostics. Measure correctness before comparing timings; include compiler, native archive hash, and isolation mode. |
| Medium | Native-library handling | [`verify_prebuilt_checksum`](../crates/feff10-sys/build.rs) deletes the checked file on mismatch, including a user-supplied library. `link_prebuilt` does not watch that library for content changes. Downloads write directly to the final cached path. | Never delete a user-owned library on verification failure; watch its path for changes. Download to a temporary file, verify, then rename atomically. Publish machine-readable compiler/BLAS/runtime metadata with archives. |
| Medium | Python errors | [`PyFeffConfig`](../crates/feff10-python/src/config.rs) passes floats directly to `Duration::from_secs_f64`; negative, nonfinite, or excessive values can panic. Callback exceptions are saved while remaining stages continue. | Validate timeouts into `ValueError`; define cancellation behavior for callback exceptions. Attach stage/exit-code/log-path attributes to Python exceptions instead of exposing only text. |
| Medium | Developer workflow/build time | [`build.rs`](../crates/feff10-sys/build.rs) copies the entire source tree and removes every object/module for a rebuild, and `make objects` runs serially. Compiler detection and backend flags are mixed with downloading and patching in one large file. | Split compiler/backend detection, source patches, and archive handling. Cache by source revision + compiler + flags + BLAS, and preserve unchanged objects. Verify dependency ordering before enabling bounded parallel compilation. |
| Medium | Performance | [`run_stage_worker`](../crates/feff10/src/pipeline.rs) formerly polled at 50 ms even without a timeout (now fixed). The global execution lock still serializes independent Worker calculations. | Benchmark worker overhead separately; allow independent directories to run concurrently only after protecting shared-directory operations and remaining in-process/fork interactions. Keep one fresh process per stage. |
| Lower | Output API/UI | [`OutputKind`](../crates/feff10/src/output.rs) classifies `feffNNNN.dat` as generic tables even though they contain geometry and specialized headers. Python table iteration clones all columns and then clones each yielded column again. | Add a structured scattering-path output type with named physical columns and units. Provide a concise CLI path summary and optional spectrum preview/export. Add contiguous NumPy export and remove redundant copies when profiling justifies it. |
| Lower | Documentation | Root, crate, C-header, and Python docs duplicate build/runtime claims, while prebuilt metadata is reported as “unknown.” | Maintain a tested platform/backend support table and executable quickstarts. Explain GNU/Intel archive choices, dynamic dependencies, Windows worker setup, and the difference between enumerated and retained paths. |

Suggested order: repair CONTROL scheduling and output validation, add the Python
worker route and wheel tests, then improve run/benchmark diagnostics and build
incrementality. Benchmark optimization changes only against verified numerical
outputs; a fast empty spectrum is not a successful performance result.

## Validation recorded for this change

- macOS arm64 source build: `cargo test --workspace` passes (177 tests/doctests,
  plus the standalone worker regression; 12 existing slow tests remain ignored).
- macOS prebuilt consumption: `scripts/test-prebuilt.sh debug` passes using a
  freshly copied source-built archive and a separate Cargo target directory.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`,
  and `git diff --check` pass.
- Actionlint validates the changed workflows with embedded shell lint disabled;
  the existing workflows have shell-style warnings. The new prebuilt test script
  passes ShellCheck separately.
- Linux x86_64 under emulation: Intel 2024.0.1 `mmtr` at `-O0`/`-O1` produces
  finite amplitudes; `-O2`/`-O3` reproduces the missing-output failure. Isolated
  runs with the same intermediate inputs yield 14 path files at `-O1`, versus
  zero at `-O2` and `-O3`. The corrected path tables are finite and the EXAFS
  spectrum is nonzero. Comparison against macOS GNU path tables has a maximum
  scaled difference of approximately `8.85e-5` (`abs(a-b)/max(1,abs(b))`).
- The Linux diagnostic archive reused the release's other objects and replaced
  the affected routine. A complete Linux source/release build and native Windows
  execution remain CI validation steps; no new release has been published.
