# Array-format header regression

This is an unreleased correction to the Fortran build used by `feff10-sys`.
Existing 0.2.3 prebuilt libraries are not repaired by rebuilding only the Rust
wrapper; new native archives are required for each supported platform.

## Failure and cause

The Linux ARM64 rexafs package check in
[run 35406642227](https://github.com/Ameyanagi/rexafs/actions/runs/35406642227)
failed in MKGTR with `Unexpected end of record while reading from gg.bin`.
`gg.bin` contains text arrays despite its extension. FMS writes each section
through `WriteComplex2D` in the upstream `IOMODS/m_iomod.f90` module. When its
optional `FileType` argument is absent, the numeric selector uses text output
but the four-character `FlType` label is uninitialized. The writer includes
those bytes in a `#DF#` header. A newline among them splits the header, and
MKGTR interprets the remaining fragment as numeric input.

An isolated ARM64 macOS reproduction used fresh stage workers from the released
0.2.3 native library and a 13-atom Cu cluster. It reproduced the exact error and
retained the generated file. Replacing only the invalid four-byte labels with
`TXT ` allowed MKGTR and all remaining stages to finish. The final first-shell
path file was byte-for-byte identical to a successful baseline. This isolates
the format-header defect; it is not a new accuracy claim for the physical model.

## Correction and regression coverage

`patch_array_format_defaults` in
[`crates/feff10-sys/build.rs`](../crates/feff10-sys/build.rs) initializes both the
format label and selector before the optional override in all five numeric 2D
writers. Explicit `TXT` and `PAD` arguments still select their original code
paths. The patch changes the build copy, preserving the upstream submodule.
It fails the build if the expected routine or format-control block is absent.

The existing Cu worker/Auto-isolation regression now requires every `gg.bin`
format header to name `TXT`, in addition to checking path geometry and finite,
nonzero numerical outputs. This catches the undefined label even when the
pipeline happens to succeed. The added assertion fails against the unchanged
0.2.3 archive because its headers contain stack bytes.

On ARM64 macOS, the corrected source build passed four Cu runs across explicit
worker and automatic isolation. The archive then passed the existing integration
suite: four tests passed, including Cu EXAFS, Cu XANES and BN XANES comparisons;
12 extended cases remained explicitly ignored. Strict workspace Clippy and
formatting checks also passed. Other platforms still require CI qualification.

The release workflow already runs this test against each freshly built native
archive through `scripts/test-prebuilt.sh`. Publication must use those corrected
archives; retrying an old binary cannot fix the underlying defect.
