use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Mapping: (pipeline_stage_name, source_file_relative_path, fortran_program_name)
const DRIVERS: &[(&str, &str, &str)] = &[
    ("rdinp", "RDINP/rdinp.f90", "rdinp"),
    ("dmdw", "DMDW/dmdw.f90", "dmdw"),
    ("atomic", "ATOM/atomic.f90", "atomic_pot"),
    ("pot", "POT/pot.f90", "ffmod1"),
    ("ldos", "LDOS/ldos.f90", "ffmod7"),
    ("screen", "SCREEN/screen.f90", "ffmod8"),
    ("crpa", "CRPA/crpa.f90", "crpa"),
    ("opconsat", "OPCONSAT/opconsat.f90", "opconsAt"),
    ("xsph", "XSPH/xsph.f90", "ffmod2"),
    ("fms", "FMS/fms.f90", "ffmod3"),
    ("mkgtr", "MKGTR/mkgtr.f90", "mkgtr"),
    ("path", "PATH/path.f90", "ffmod4"),
    ("genfmt", "GENFMT/genfmt.f90", "ffmod5"),
    ("ff2x", "FF2X/ff2x.f90", "ffmod6"),
    ("sfconv", "SFCONV/sfconv.f90", "ffmod9"),
    ("compton", "COMPTON/compton.f90", "compton"),
    ("eels", "EELS/eels.f90", "eelsmod"),
    ("rhorrp", "RHORRP/rhorrp.f90", "rhorrp_prog"),
];

/// Which BLAS/LAPACK implementation was detected.
enum BlasType {
    Mkl {
        lib_dir: PathBuf,
        interface: String,
    },
    OpenBlas,
    SystemBlas,
    Accelerate,
    None,
}

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let feff_src = manifest_dir.join("../../feff10/src");

    // 1. Detect Fortran compiler
    let (compiler, flags) = detect_compiler();
    eprintln!("feff10-sys: using Fortran compiler: {compiler}");
    eprintln!("feff10-sys: flags: {flags}");

    // 2. Copy source tree to OUT_DIR (don't pollute submodule)
    let build_src = out_dir.join("feff10-src");
    copy_dir_recursive(&feff_src, &build_src);

    // 3. Copy PAR/sequential.src -> PAR/parallel.f90 and patch par_stop
    let par_dir = build_src.join("PAR");
    let seq_src = par_dir.join("sequential.src");
    let parallel_f90 = par_dir.join("parallel.f90");
    if seq_src.exists() {
        fs::copy(&seq_src, &parallel_f90).expect("Failed to copy PAR/sequential.src");
    }
    patch_par_stop(&parallel_f90);

    // 4. Patch driver files: convert `program` → `subroutine ... bind(C)`
    //    This transforms the 18 Fortran executables into library entry points
    //    callable from Rust via FFI.
    patch_drivers_for_library(&build_src);

    // 5. Detect BLAS/LAPACK and generate Compiler.mk
    let (blas_ldflags, deptype, blas_type) = detect_blas_full(&compiler);
    let compiler_mk = build_src.join("Compiler.mk");
    {
        let mut f = fs::File::create(&compiler_mk).expect("Failed to create Compiler.mk");
        writeln!(f, "F90 = {compiler}").unwrap();
        writeln!(f, "FLAGS = {flags}").unwrap();
        writeln!(f, "MPIF90 = mpif90").unwrap();
        writeln!(f, "MPIFLAGS = -O3").unwrap();
        // LDFLAGS not used for object-only compilation, but Makefile requires it
        writeln!(f, "LDFLAGS = {blas_ldflags}").unwrap();
        writeln!(f, "FCINCLUDE =").unwrap();
        writeln!(f, "DEPTYPE = {deptype}").unwrap();
        drop(f);
    }

    // 6. Clean stale .o and .mod files from previous builds
    //    (prevents duplicate symbols when switching compilers)
    for f in find_files_recursive(&build_src, "o") {
        let _ = fs::remove_file(f);
    }
    for f in find_files_recursive(&build_src, "mod") {
        let _ = fs::remove_file(f);
    }

    // 7. Append `objects` target to Makefile (compiles all .o files without linking)
    append_objects_target(&build_src);

    // 8. Run `make objects`
    run_make_objects(&build_src, &compiler, &flags);

    // 9. Collect all .o files and create libfeff10_raw.a
    let raw_archive = out_dir.join("libfeff10_raw.a");
    create_archive_from_objects(&build_src, &raw_archive);

    // 10. Create final archive — merge MKL/Intel runtime if applicable
    let final_archive = out_dir.join("libfeff10.a");
    let mut merge_libs = Vec::new();

    if let BlasType::Mkl {
        lib_dir,
        interface,
    } = &blas_type
    {
        merge_libs.push(lib_dir.join(format!("lib{interface}.a")));
        merge_libs.push(lib_dir.join("libmkl_sequential.a"));
        merge_libs.push(lib_dir.join("libmkl_core.a"));
    }

    // For Intel compiler, merge Intel Fortran runtime into the archive
    // so the final binary doesn't need LD_LIBRARY_PATH pointing to oneAPI.
    // Use _pic variants for PIC-compatible static linking (required for PIE executables).
    if compiler.contains("ifx") || compiler.contains("ifort") {
        if let Some(compiler_dir) = Path::new(&compiler).parent().and_then(|p| p.parent()) {
            let lib_dir = compiler_dir.join("lib");
            // Prefer _pic variants (PIC-compatible), fall back to regular
            for (pic, regular) in &[
                ("libifcore_pic.a", "libifcore.a"),
                ("libimf.a", "libimf.a"),
                ("libsvml.a", "libsvml.a"),
                ("libirc.a", "libirc.a"),
            ] {
                let pic_path = lib_dir.join(pic);
                let reg_path = lib_dir.join(regular);
                if pic_path.exists() {
                    merge_libs.push(pic_path);
                } else if reg_path.exists() {
                    merge_libs.push(reg_path);
                }
            }
        }
    }

    if !merge_libs.is_empty() && cfg!(target_os = "linux") {
        eprintln!(
            "feff10-sys: merging {} external archives into libfeff10.a",
            merge_libs.len()
        );
        merge_archives(&raw_archive, &final_archive, &merge_libs);
    } else {
        // No merge needed — just rename
        fs::rename(&raw_archive, &final_archive)
            .or_else(|_| fs::copy(&raw_archive, &final_archive).map(|_| ()))
            .expect("Failed to create final archive");
    }

    // 11. Emit cargo link directives
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=feff10");

    // Fortran runtime (dynamic linking — part of the system)
    emit_fortran_runtime_links(&compiler);

    // BLAS (only if NOT merged into the archive)
    match &blas_type {
        BlasType::Mkl { .. } if cfg!(target_os = "linux") => {
            // Already merged into libfeff10.a via ld -r
        }
        BlasType::Mkl {
            lib_dir,
            interface,
        } => {
            // macOS or other: can't merge, link separately
            println!("cargo:rustc-link-search=native={}", lib_dir.display());
            println!("cargo:rustc-link-lib=static={interface}");
            println!("cargo:rustc-link-lib=static=mkl_sequential");
            println!("cargo:rustc-link-lib=static=mkl_core");
        }
        BlasType::OpenBlas => {
            println!("cargo:rustc-link-lib=openblas");
        }
        BlasType::SystemBlas => {
            println!("cargo:rustc-link-lib=lapack");
            println!("cargo:rustc-link-lib=blas");
        }
        BlasType::Accelerate => {
            println!("cargo:rustc-link-lib=framework=Accelerate");
        }
        BlasType::None => {}
    }

    // Common system libraries
    println!("cargo:rustc-link-lib=pthread");
    println!("cargo:rustc-link-lib=m");
    if cfg!(target_os = "linux") {
        println!("cargo:rustc-link-lib=dl");
    }

    // 12. Expose build metadata to dependent crates
    let blas_name = match &blas_type {
        BlasType::Mkl { .. } => "MKL",
        BlasType::OpenBlas => "OpenBLAS",
        BlasType::SystemBlas => "system BLAS",
        BlasType::Accelerate => "Accelerate",
        BlasType::None => "naive (built-in)",
    };
    println!("cargo:FC={compiler}");
    println!("cargo:FFLAGS={flags}");
    println!("cargo:BLAS={blas_name}");

    // FEFF10 upstream commit from the git submodule
    let feff10_dir = manifest_dir.join("../../feff10");
    let feff10_commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(&feff10_dir)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:FEFF10_COMMIT={feff10_commit}");

    // 13. Emit rerun-if-changed directives
    println!("cargo:rerun-if-env-changed=FEFF_FC");
    println!("cargo:rerun-if-env-changed=FC");
    println!("cargo:rerun-if-env-changed=FEFF_FFLAGS");
    println!("cargo:rerun-if-env-changed=FEFF_BLAS");
    println!("cargo:rerun-if-env-changed=FEFF_NO_NATIVE");
    println!("cargo:rerun-if-env-changed=FEFF_LTO");
    println!("cargo:rerun-if-env-changed=MKLROOT");
    println!("cargo:rerun-if-env-changed=LD_LIBRARY_PATH");

    println!(
        "cargo:rerun-if-changed={}",
        feff_src.join("Makefile").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        feff_src.join("Compiler.mk.default").display()
    );

    let src_dirs = [
        "ATOM", "BAND", "COMMON", "COMPTON", "CRPA", "DEBYE", "DMDW", "EELS", "EELSMDFF",
        "ERRORMODS", "EXCH", "FF2X", "FMS", "FOVRG", "FULLSPECTRUM", "GENFMT", "IOMODS",
        "KSPACE", "LDOS", "MATH", "MKGTR", "MODS", "PAR", "PATH", "POT", "RDINP", "RHORRP",
        "RIXS", "SCREEN", "SELF", "SFCONV", "TDLDA", "XSPH", "INPGEN", "HEADERS", "DEP",
    ];
    for dir in &src_dirs {
        println!("cargo:rerun-if-changed={}", feff_src.join(dir).display());
    }
}

// ---------------------------------------------------------------------------
// Fortran driver patching
// ---------------------------------------------------------------------------

/// Patch all 18 driver files in-place (in the OUT_DIR copy):
/// - `program NAME` → `subroutine feff_STAGE() bind(C, name="feff_STAGE")`
/// - `end program [NAME]` / bare `end` → `end subroutine feff_STAGE`
/// - bare `stop` → `return`
fn patch_drivers_for_library(build_src: &Path) {
    for &(stage, src_rel, fortran_name) in DRIVERS {
        let src_path = build_src.join(src_rel);
        if !src_path.exists() {
            panic!(
                "feff10-sys: driver source not found: {}",
                src_path.display()
            );
        }

        let content = fs::read_to_string(&src_path)
            .unwrap_or_else(|e| panic!("Failed to read {}: {e}", src_path.display()));

        let patched = patch_driver_content(&content, stage, fortran_name);

        fs::write(&src_path, patched)
            .unwrap_or_else(|e| panic!("Failed to write {}: {e}", src_path.display()));

        eprintln!("feff10-sys: patched {src_rel} → feff_{stage}()");
    }
}

/// Apply patching rules to a single driver file's content.
fn patch_driver_content(content: &str, stage: &str, fortran_name: &str) -> String {
    let subroutine_decl = format!("      subroutine feff_{stage}() bind(C, name=\"feff_{stage}\")");
    let end_subroutine = format!("      end subroutine feff_{stage}");

    let mut result: Vec<String> = Vec::new();
    let mut found_end_program = false;

    for line in content.lines() {
        let trimmed = line.trim();
        let code_part = trimmed.split('!').next().unwrap_or("").trim();
        let tokens: Vec<&str> = code_part.split_whitespace().collect();

        // Rule 1: program NAME → subroutine feff_STAGE() bind(C)
        if tokens.len() >= 2
            && tokens[0].eq_ignore_ascii_case("program")
            && tokens[1].eq_ignore_ascii_case(fortran_name)
        {
            result.push(subroutine_decl.clone());
            continue;
        }

        // Rule 2: end program [NAME] → end subroutine feff_STAGE
        if tokens.len() >= 2
            && tokens[0].eq_ignore_ascii_case("end")
            && tokens[1].eq_ignore_ascii_case("program")
        {
            result.push(end_subroutine.clone());
            found_end_program = true;
            continue;
        }

        // Rule 3: bare `stop` → `return`
        if code_part.eq_ignore_ascii_case("stop") {
            let indent = &line[..line.len() - line.trim_start().len()];
            result.push(format!("{indent}return"));
            continue;
        }

        result.push(line.to_string());
    }

    // Rule 4: If no `end program` was found, replace the last bare `end`
    if !found_end_program {
        let mut replaced = false;
        for i in (0..result.len()).rev() {
            let code_part = result[i].trim().split('!').next().unwrap_or("").trim();
            if code_part.eq_ignore_ascii_case("end") {
                result[i] = end_subroutine.clone();
                replaced = true;
                break;
            }
        }
        if !replaced {
            panic!("feff10-sys: could not find terminal `end` in driver for stage {stage}");
        }
    }

    let mut output = result.join("\n");
    if content.ends_with('\n') {
        output.push('\n');
    }
    output
}

/// Patch PAR/parallel.f90: replace `stop ' '` in par_stop with `return`.
/// This makes par_stop non-fatal so execution returns to the caller.
fn patch_par_stop(parallel_f90: &Path) {
    if !parallel_f90.exists() {
        return;
    }
    let content = fs::read_to_string(parallel_f90)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", parallel_f90.display()));

    // Replace `stop ' '` with `return` inside the par_stop subroutine
    let patched = content.replace("stop ' '", "return");

    fs::write(parallel_f90, patched)
        .unwrap_or_else(|e| panic!("Failed to write {}: {e}", parallel_f90.display()));

    eprintln!("feff10-sys: patched PAR/parallel.f90 (par_stop: stop → return)");
}

// ---------------------------------------------------------------------------
// Build system (Makefile + make + ar)
// ---------------------------------------------------------------------------

/// Append an `objects` target to the copied Makefile.
/// This target compiles all .o files needed by the 18 pipeline stages without linking.
fn append_objects_target(build_src: &Path) {
    let makefile = build_src.join("Makefile");
    let mut f = fs::OpenOptions::new()
        .append(true)
        .open(&makefile)
        .expect("Failed to open Makefile for appending");

    let stage_names: Vec<&str> = DRIVERS.iter().map(|&(name, _, _)| name).collect();
    let targets = stage_names.join(" ");

    writeln!(f).unwrap();
    writeln!(
        f,
        "# Library build target (added by build.rs for static library compilation)"
    )
    .unwrap();
    writeln!(f, "LIBRARY_TARGETS = {targets}").unwrap();
    writeln!(
        f,
        "ALL_LIB_OBJ = $(sort $(foreach exe,$(LIBRARY_TARGETS),$($(exe)_MODULES) $($(exe)_OBJ)))"
    )
    .unwrap();
    writeln!(f, "objects: $(ALL_LIB_OBJ)").unwrap();

    eprintln!("feff10-sys: appended `objects` target to Makefile");
}

/// Run `make objects` to compile all Fortran source files into .o files.
fn run_make_objects(build_src: &Path, compiler: &str, flags: &str) {
    eprintln!("feff10-sys: running make objects in {}", build_src.display());

    let mut cmd = Command::new("make");
    cmd.args([
        &format!("F90={compiler}"),
        &format!("FLAGS={flags}"),
        // Always pass -DFEFF so that #ifdef FEFF blocks are active.
        // The Makefile only sets FPPTASK for non-gfortran compilers,
        // but we need it for all compilers in library mode.
        "FPPTASK=-DFEFF",
        "objects",
    ])
    .current_dir(build_src)
    .env("MAKEFLAGS", ""); // Clear inherited flags

    // On macOS, flang-new may need SDKROOT
    if cfg!(target_os = "macos") && compiler.contains("flang") {
        if env::var("SDKROOT").is_err() {
            if let Ok(output) = Command::new("xcrun").arg("--show-sdk-path").output() {
                if output.status.success() {
                    let sdk = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    cmd.env("SDKROOT", sdk);
                }
            }
        }
    }

    // On Linux with Intel oneAPI, propagate library paths
    if cfg!(target_os = "linux") && compiler.contains("oneapi") {
        let mut ld_paths = Vec::new();
        if let Some(mkl_root) = find_mkl_root() {
            ld_paths.push(format!("{}/lib/intel64", mkl_root.display()));
        }
        if let Some(compiler_dir) = Path::new(compiler).parent().and_then(|p| p.parent()) {
            let compiler_lib = compiler_dir.join("lib");
            if compiler_lib.is_dir() {
                ld_paths.push(compiler_lib.display().to_string());
            }
        }
        if !ld_paths.is_empty() {
            let existing = env::var("LD_LIBRARY_PATH").unwrap_or_default();
            let new_path = if existing.is_empty() {
                ld_paths.join(":")
            } else {
                format!("{}:{existing}", ld_paths.join(":"))
            };
            cmd.env("LD_LIBRARY_PATH", &new_path);
            cmd.env("LIBRARY_PATH", &new_path);
        }
    }

    let status = cmd.status().expect("Failed to run make. Is `make` installed?");
    if !status.success() {
        panic!(
            "feff10-sys: make objects failed with exit code {:?}",
            status.code()
        );
    }
}

/// Collect all .o files from the build tree and create a static archive.
fn create_archive_from_objects(build_src: &Path, archive_path: &Path) {
    let objects = find_files_recursive(build_src, "o");
    if objects.is_empty() {
        panic!("feff10-sys: no .o files found in {}", build_src.display());
    }

    eprintln!(
        "feff10-sys: archiving {} object files into {}",
        objects.len(),
        archive_path.display()
    );

    // Remove existing archive to avoid stale entries
    let _ = fs::remove_file(archive_path);

    // On Windows, passing 435+ absolute-path object files as args exceeds the
    // command line limit (~32K chars). Use relative paths from build_src to keep
    // the command short (e.g. "ATOM/akeato.o" instead of the full absolute path).
    let use_relative = cfg!(target_os = "windows");
    let rel_objects: Vec<PathBuf> = if use_relative {
        objects
            .iter()
            .map(|o| o.strip_prefix(build_src).unwrap_or(o).to_path_buf())
            .collect()
    } else {
        objects
    };

    let mut cmd = Command::new("ar");
    cmd.arg("rcs").arg(archive_path);
    if use_relative {
        cmd.current_dir(build_src);
    }
    cmd.args(&rel_objects);

    let status = cmd
        .status()
        .expect("Failed to run ar. Is `ar` (binutils) installed?");

    if !status.success() {
        panic!("feff10-sys: ar rcs failed");
    }
}

/// Merge the FEFF archive with external static libraries (MKL, Intel runtime)
/// using `ld -r` (partial/incremental linking) to resolve circular dependencies.
///
/// This produces a single relocatable object that contains:
/// - ALL FEFF code (via --whole-archive)
/// - Only the needed symbols from external libs (via --start-group)
fn merge_archives(raw_archive: &Path, final_archive: &Path, extra_libs: &[PathBuf]) {
    let combined_o = final_archive.with_extension("o");

    // Verify all input archives exist
    for lib in extra_libs {
        if !lib.exists() {
            panic!(
                "feff10-sys: library not found for merge: {}",
                lib.display()
            );
        }
    }

    let mut cmd = Command::new("ld");
    cmd.arg("-r")
        .arg("--whole-archive")
        .arg(raw_archive)
        .arg("--no-whole-archive")
        .arg("--start-group");

    for lib in extra_libs {
        cmd.arg(lib);
    }

    cmd.arg("--end-group").arg("-o").arg(&combined_o);

    eprintln!("feff10-sys: running ld -r to merge archives");
    let status = cmd.status().expect("Failed to run ld for archive merge");
    if !status.success() {
        panic!("feff10-sys: ld -r failed. Cannot merge archives.");
    }

    // Wrap the combined object in an archive
    let _ = fs::remove_file(final_archive);
    let status = Command::new("ar")
        .args(["rcs"])
        .arg(final_archive)
        .arg(&combined_o)
        .status()
        .expect("Failed to run ar");

    if !status.success() {
        panic!("feff10-sys: ar rcs failed for final archive");
    }

    // Clean up intermediate .o
    let _ = fs::remove_file(&combined_o);

    // Report size
    if let Ok(meta) = fs::metadata(final_archive) {
        let size_mb = meta.len() as f64 / (1024.0 * 1024.0);
        eprintln!(
            "feff10-sys: final archive size: {:.1} MB",
            size_mb
        );
    }
}

/// Emit cargo link directives for the Fortran runtime.
fn emit_fortran_runtime_links(compiler: &str) {
    if compiler.contains("gfortran") {
        println!("cargo:rustc-link-lib=gfortran");
    } else if compiler.contains("ifx") || compiler.contains("ifort") {
        // Intel runtime is merged into the archive on Linux.
        // On other platforms, link dynamically.
        if !cfg!(target_os = "linux") {
            if let Some(compiler_dir) = Path::new(compiler).parent().and_then(|p| p.parent()) {
                let lib_dir = compiler_dir.join("lib");
                if lib_dir.is_dir() {
                    println!("cargo:rustc-link-search=native={}", lib_dir.display());
                }
            }
            println!("cargo:rustc-link-lib=ifcore");
            println!("cargo:rustc-link-lib=imf");
            println!("cargo:rustc-link-lib=svml");
            println!("cargo:rustc-link-lib=irc");
        }
    } else if compiler.contains("flang") {
        // LLVM Flang runtime — find the clang resource directory
        let found = find_flang_runtime(compiler);
        if found {
            println!("cargo:rustc-link-lib=static=flang_rt.runtime");
        } else {
            // Static library not found — try dynamic linking as fallback
            eprintln!("feff10-sys: flang_rt.runtime.a not found, trying dynamic linking");
            println!("cargo:rustc-link-lib=flang_rt.runtime");
        }
    }
}

/// Find and emit link search path for LLVM Flang runtime library.
/// Searches clang resource directories for libflang_rt.runtime.a.
fn find_flang_runtime(compiler: &str) -> bool {
    // Try the actual compiler first (e.g. flang-new-20), then common names
    let mut candidates: Vec<&str> = vec![compiler];
    let extra = ["flang-new", "flang"];
    for c in &extra {
        if *c != compiler {
            candidates.push(c);
        }
    }
    for flang_cmd in &candidates {
        if let Ok(output) = Command::new(flang_cmd).arg("--print-resource-dir").output() {
            if output.status.success() {
                let resource_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
                // Check lib/linux (common layout) and lib/ (alternative)
                for subdir in &["lib/linux", "lib"] {
                    let lib_dir = Path::new(&resource_dir).join(subdir);
                    if lib_dir.join("libflang_rt.runtime.a").exists() {
                        println!("cargo:rustc-link-search=native={}", lib_dir.display());
                        return true;
                    }
                }
            }
        }
    }
    // Fallback: search common paths
    let search_paths = [
        // Standard clang resource dir layout
        "/usr/lib/clang/22/lib/linux",
        "/usr/lib/clang/21/lib/linux",
        "/usr/lib/clang/20/lib/linux",
        "/usr/lib/clang/19/lib/linux",
        // Per-target layout (used by some distributions)
        "/usr/lib/clang/20/lib/x86_64-pc-linux-gnu",
        "/usr/lib/clang/21/lib/x86_64-pc-linux-gnu",
        "/usr/lib/clang/22/lib/x86_64-pc-linux-gnu",
        // LLVM versioned install paths
        "/usr/lib/llvm-20/lib",
        "/usr/lib/llvm-21/lib",
        "/usr/lib/llvm-22/lib",
    ];
    for path in &search_paths {
        let p = Path::new(path);
        if p.join("libflang_rt.runtime.a").exists() {
            println!("cargo:rustc-link-search=native={}", p.display());
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Compiler and BLAS detection (mostly unchanged from previous version)
// ---------------------------------------------------------------------------

fn detect_compiler() -> (String, String) {
    if let Ok(fc) = env::var("FEFF_FC") {
        let flags = env::var("FEFF_FFLAGS").unwrap_or_else(|_| default_flags_for(&fc));
        return (fc, flags);
    }
    if let Ok(fc) = env::var("FC") {
        let flags = env::var("FEFF_FFLAGS").unwrap_or_else(|_| default_flags_for(&fc));
        return (fc, flags);
    }

    // Probe Intel oneAPI paths first (ifx/ifort are often not on PATH)
    if cfg!(target_os = "linux") {
        for path in &[
            "/opt/intel/oneapi/compiler/latest/bin/ifx",
            "/opt/intel/oneapi/compiler/latest/bin/ifort",
        ] {
            if Path::new(path).exists() {
                let basename = Path::new(path).file_name().unwrap().to_str().unwrap();
                let flags =
                    env::var("FEFF_FFLAGS").unwrap_or_else(|_| default_flags_for(basename));
                eprintln!("feff10-sys: found Intel compiler at {path}");
                return (path.to_string(), flags);
            }
        }
    }

    for candidate in &["ifx", "ifort", "gfortran", "flang-new"] {
        if which::which(candidate).is_ok() {
            let flags = env::var("FEFF_FFLAGS").unwrap_or_else(|_| default_flags_for(candidate));
            return (candidate.to_string(), flags);
        }
    }

    panic!(
        "feff10-sys: No Fortran compiler found. \
         Install gfortran, ifx, or flang-new, or set FEFF_FC env var."
    );
}

fn default_flags_for(compiler: &str) -> String {
    // FEFF_MARCH: explicit arch target (e.g. "x86-64-v3")
    // FEFF_PORTABLE: shorthand for -march=x86-64-v3 (good for distributing binaries)
    // FEFF_NO_NATIVE: disable -march entirely
    // Default: -march=native (best for local builds)
    let march = if let Ok(arch) = env::var("FEFF_MARCH") {
        format!(" -march={arch}")
    } else if env::var("FEFF_PORTABLE").is_ok() {
        " -march=x86-64-v3".to_string()
    } else if env::var("FEFF_NO_NATIVE").is_ok() {
        String::new()
    } else {
        " -march=native".to_string()
    };

    let lto = env::var("FEFF_LTO").is_ok();

    // Intel compilers use -xHost/-march=core-avx2 instead of -march=native/-march=x86-64-v3
    let intel_arch = if let Ok(arch) = env::var("FEFF_MARCH") {
        format!(" -march={arch}")
    } else if env::var("FEFF_PORTABLE").is_ok() {
        " -march=core-avx2".to_string()
    } else if env::var("FEFF_NO_NATIVE").is_ok() {
        String::new()
    } else {
        " -xHost".to_string()
    };

    // -fPIC is needed because Rust links a PIE executable
    if compiler.contains("gfortran") {
        let lto_flag = if lto { " -flto=auto" } else { "" };
        format!("-ffree-line-length-none -cpp -O3 -fPIC -fallow-argument-mismatch{march}{lto_flag}")
    } else if compiler.contains("ifx") {
        let lto_flag = if lto { " -ipo" } else { "" };
        // -no-vec: workaround for ifx 2025.3 ICE in VPlan vectorizer on ff2chijas.f90
        format!("-O3 -fpp -fPIC{intel_arch} -no-vec{lto_flag}")
    } else if compiler.contains("ifort") {
        let lto_flag = if lto { " -ipo" } else { "" };
        format!("-O3 -fPIC{intel_arch}{lto_flag}")
    } else if compiler.contains("flang") {
        let lto_flag = if lto { " -flto" } else { "" };
        format!("-O3 -cpp -fPIC -fno-automatic{march}{lto_flag}")
    } else {
        "-O3 -fPIC".to_string()
    }
}

/// Detect BLAS/LAPACK. Returns (makefile_ldflags, deptype, blas_type).
fn detect_blas_full(compiler: &str) -> (String, String, BlasType) {
    if env::var("FEFF_BLAS").as_deref() == Ok("none") {
        eprintln!("feff10-sys: BLAS disabled (FEFF_BLAS=none), using naive MATH/lu.f90");
        return (String::new(), String::new(), BlasType::None);
    }

    if let Ok(blas) = env::var("FEFF_BLAS") {
        eprintln!("feff10-sys: using FEFF_BLAS={blas}");
        // Can't determine type from user string — treat as generic
        return (blas, "_MKL".to_string(), BlasType::None);
    }

    // macOS: Accelerate framework
    if cfg!(target_os = "macos") {
        let framework_dir = Path::new("/System/Library/Frameworks/Accelerate.framework");
        if framework_dir.is_dir() {
            eprintln!("feff10-sys: using Apple Accelerate for BLAS/LAPACK");
            return (
                "-framework Accelerate".to_string(),
                "_MKL".to_string(),
                BlasType::Accelerate,
            );
        }
    }

    // Intel MKL — preferred on Linux
    if cfg!(target_os = "linux") {
        if let Some((mkl_ldflags, lib_dir, interface)) = detect_mkl_full(compiler) {
            return (
                mkl_ldflags,
                "_MKL".to_string(),
                BlasType::Mkl { lib_dir, interface },
            );
        }
    }

    // OpenBLAS fallback
    if cfg!(target_os = "linux") {
        if let Ok(output) = Command::new("pkg-config").args(["--libs", "openblas"]).output() {
            if output.status.success() {
                let libs = String::from_utf8_lossy(&output.stdout).trim().to_string();
                eprintln!("feff10-sys: using OpenBLAS via pkg-config: {libs}");
                return (libs, "_MKL".to_string(), BlasType::OpenBlas);
            }
        }
        if Path::new("/usr/lib/x86_64-linux-gnu/libopenblas.so").exists()
            || Path::new("/usr/lib64/libopenblas.so").exists()
            || Path::new("/usr/lib/libopenblas.so").exists()
        {
            eprintln!("feff10-sys: using OpenBLAS (-lopenblas)");
            return (
                "-lopenblas".to_string(),
                "_MKL".to_string(),
                BlasType::OpenBlas,
            );
        }
        // System LAPACK/BLAS
        if Path::new("/usr/lib/x86_64-linux-gnu/liblapack.so").exists()
            || Path::new("/usr/lib64/liblapack.so").exists()
        {
            eprintln!("feff10-sys: using system LAPACK/BLAS");
            return (
                "-llapack -lblas".to_string(),
                "_MKL".to_string(),
                BlasType::SystemBlas,
            );
        }
    }

    eprintln!("feff10-sys: no optimized BLAS found, using naive MATH/lu.f90");
    (String::new(), String::new(), BlasType::None)
}

/// Detect MKL and return (ldflags_for_makefile, lib_dir, interface_lib_name).
fn detect_mkl_full(compiler: &str) -> Option<(String, PathBuf, String)> {
    let mkl_root = find_mkl_root()?;
    let lib_dir = mkl_root.join("lib/intel64");

    if !lib_dir.join("libmkl_core.so").exists() && !lib_dir.join("libmkl_core.a").exists() {
        eprintln!(
            "feff10-sys: MKL root found at {} but libraries missing",
            mkl_root.display()
        );
        return None;
    }

    let interface = if compiler.contains("ifx") || compiler.contains("ifort") {
        "mkl_intel_lp64"
    } else {
        "mkl_gf_lp64"
    };

    let ldflags = format!(
        "-L{lib} -Wl,--start-group {lib}/lib{interface}.a {lib}/libmkl_sequential.a {lib}/libmkl_core.a -Wl,--end-group -lpthread -lm -ldl",
        lib = lib_dir.display(),
    );

    eprintln!(
        "feff10-sys: using MKL ({interface}) at {}",
        mkl_root.display()
    );
    Some((ldflags, lib_dir, interface.to_string()))
}

fn find_mkl_root() -> Option<PathBuf> {
    if let Ok(root) = env::var("MKLROOT") {
        let path = PathBuf::from(&root);
        if path.is_dir() {
            return Some(path);
        }
    }
    for path in &["/opt/intel/oneapi/mkl/latest"] {
        let p = PathBuf::from(path);
        if p.is_dir() {
            return Some(p);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Find all files with a given extension recursively.
fn find_files_recursive(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    fn walk(dir: &Path, ext: &str, files: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, ext, files);
                } else if path.extension().map_or(false, |e| e == ext) {
                    files.push(path);
                }
            }
        }
    }
    walk(dir, extension, &mut files);
    files
}

/// Recursively copy a directory tree.
fn copy_dir_recursive(src: &Path, dst: &Path) {
    if !dst.exists() {
        fs::create_dir_all(dst).unwrap();
    }
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        let name = entry.file_name();
        // Skip directories not needed for build
        if name == ".git"
            || name == ".github"
            || name == "doc"
            || name == "examples"
            || name == "Bugs"
            || name == "windows"
            || name == "windowsNoMkl"
            || name == "project"
        {
            continue;
        }

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else if ty.is_file() {
            fs::copy(&src_path, &dst_path).unwrap_or_else(|e| {
                panic!(
                    "Failed to copy {} -> {}: {e}",
                    src_path.display(),
                    dst_path.display()
                );
            });
        }
    }
}
