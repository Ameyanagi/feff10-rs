use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    if build_src.exists() {
        // Only re-copy if source is newer
        // For simplicity, always copy - make will handle incremental compilation
    }
    copy_dir_recursive(&feff_src, &build_src);

    // 3. Detect optimized BLAS/LAPACK and generate Compiler.mk
    let mut ldflags = ldflags_for(&compiler);
    let (blas_ldflags, deptype) = detect_blas(&compiler);
    if !blas_ldflags.is_empty() {
        if !ldflags.is_empty() {
            ldflags.push(' ');
        }
        ldflags.push_str(&blas_ldflags);
    }

    let compiler_mk = build_src.join("Compiler.mk");
    let mut f = fs::File::create(&compiler_mk).expect("Failed to create Compiler.mk");
    writeln!(f, "F90 = {compiler}").unwrap();
    writeln!(f, "FLAGS = {flags}").unwrap();
    writeln!(f, "MPIF90 = mpif90").unwrap();
    writeln!(f, "MPIFLAGS = -O3").unwrap();
    writeln!(f, "LDFLAGS = {ldflags}").unwrap();
    writeln!(f, "FCINCLUDE =").unwrap();
    writeln!(f, "DEPTYPE = {deptype}").unwrap();
    drop(f);

    // 4. Copy PAR/sequential.src -> PAR/parallel.f90
    let par_dir = build_src.join("PAR");
    let seq_src = par_dir.join("sequential.src");
    let parallel_f90 = par_dir.join("parallel.f90");
    if seq_src.exists() {
        fs::copy(&seq_src, &parallel_f90).expect("Failed to copy PAR/sequential.src");
    }

    // 5. Create bin directory and invoke make
    let bin_dir = out_dir.join("bin");
    fs::create_dir_all(&bin_dir).expect("Failed to create bin directory");

    let executables = [
        "rdinp", "dmdw", "atomic", "pot", "ldos", "screen", "crpa", "opconsat", "xsph", "fms",
        "mkgtr", "path", "genfmt", "ff2x", "sfconv", "compton", "eels", "rhorrp",
    ];

    let exec_dir = format!("{}/", bin_dir.display());

    // Build executables one at a time - Fortran module dependencies
    // prevent reliable parallel compilation across different targets.
    // Each individual target's compilation is serial (make handles
    // intra-target ordering via DEP/dependencies.mk).
    let mut make_args = vec![
        format!("F90={compiler}"),
        format!("FLAGS={flags}"),
        format!("EXECDIR={exec_dir}"),
    ];
    make_args.extend(executables.iter().map(|s| s.to_string()));

    eprintln!("feff10-sys: running make in {}", build_src.display());

    let mut cmd = Command::new("make");
    cmd.args(&make_args)
        .current_dir(&build_src)
        .env("MAKEFLAGS", ""); // Clear inherited flags to avoid conflicts

    // On macOS, flang-new from Nix/Homebrew may need SDKROOT to find -lSystem
    if cfg!(target_os = "macos") && compiler.contains("flang") {
        if env::var("SDKROOT").is_err() {
            if let Ok(output) = Command::new("xcrun").arg("--show-sdk-path").output() {
                if output.status.success() {
                    let sdk = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    eprintln!("feff10-sys: setting SDKROOT={sdk} for flang-new on macOS");
                    cmd.env("SDKROOT", sdk);
                }
            }
        }
    }

    // On Linux with Intel oneAPI, set up library paths for the linker.
    // When ifx is invoked via full path (without setvars.sh), the compiler
    // driver resolves MKL/runtime libs itself, but LD_LIBRARY_PATH may be
    // needed for dynamic linking at build time.
    if cfg!(target_os = "linux") && compiler.contains("oneapi") {
        let mut ld_paths = Vec::new();
        if let Some(mkl_root) = find_mkl_root() {
            ld_paths.push(format!("{}/lib/intel64", mkl_root.display()));
        }
        // Add compiler runtime lib path
        if let Some(compiler_dir) = Path::new(&compiler).parent().and_then(|p| p.parent()) {
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
            eprintln!("feff10-sys: setting LD_LIBRARY_PATH for Intel oneAPI");
            cmd.env("LD_LIBRARY_PATH", &new_path);
            cmd.env("LIBRARY_PATH", &new_path);
        }
    }

    let status = cmd.status().expect("Failed to run make. Is `make` installed?");

    if !status.success() {
        panic!(
            "feff10-sys: make failed with exit code {:?}",
            status.code()
        );
    }

    // Verify executables were built
    for exe in &executables {
        let exe_path = bin_dir.join(exe);
        if !exe_path.exists() {
            panic!("feff10-sys: expected executable not found: {}", exe_path.display());
        }
    }

    // 6. Generate paths.rs
    let paths_rs = out_dir.join("paths.rs");
    let mut f = fs::File::create(&paths_rs).unwrap();
    writeln!(
        f,
        r#"const FEFF_BIN_DIR: &str = "{}";"#,
        bin_dir.display().to_string().replace('\\', "\\\\")
    )
    .unwrap();
    drop(f);

    // 7. Emit cargo directives
    println!("cargo:rerun-if-env-changed=FEFF_FC");
    println!("cargo:rerun-if-env-changed=FC");
    println!("cargo:rerun-if-env-changed=FEFF_FFLAGS");
    println!("cargo:rerun-if-env-changed=FEFF_BLAS");
    println!("cargo:rerun-if-env-changed=FEFF_NO_NATIVE");
    println!("cargo:rerun-if-env-changed=FEFF_LTO");
    println!("cargo:rerun-if-env-changed=MKLROOT");
    println!("cargo:rerun-if-env-changed=LD_LIBRARY_PATH");

    // Track the key build files
    println!(
        "cargo:rerun-if-changed={}",
        feff_src.join("Makefile").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        feff_src.join("Compiler.mk.default").display()
    );

    // Track source directories
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

/// Detect the Fortran compiler and appropriate flags.
fn detect_compiler() -> (String, String) {
    // Check for explicit override
    if let Ok(fc) = env::var("FEFF_FC") {
        let flags = env::var("FEFF_FFLAGS")
            .unwrap_or_else(|_| default_flags_for(&fc));
        return (fc, flags);
    }
    if let Ok(fc) = env::var("FC") {
        let flags = env::var("FEFF_FFLAGS")
            .unwrap_or_else(|_| default_flags_for(&fc));
        return (fc, flags);
    }

    // Probe for compilers in order of preference
    for candidate in &["gfortran", "ifx", "flang-new", "ifort"] {
        if which::which(candidate).is_ok() {
            let flags = env::var("FEFF_FFLAGS")
                .unwrap_or_else(|_| default_flags_for(candidate));
            return (candidate.to_string(), flags);
        }
    }

    // Probe standard Intel oneAPI installation paths (ifx may not be in PATH
    // without sourcing setvars.sh)
    if cfg!(target_os = "linux") {
        for path in &[
            "/opt/intel/oneapi/compiler/latest/bin/ifx",
            "/opt/intel/oneapi/compiler/latest/bin/ifort",
        ] {
            if Path::new(path).exists() {
                let basename = Path::new(path).file_name().unwrap().to_str().unwrap();
                let flags = env::var("FEFF_FFLAGS")
                    .unwrap_or_else(|_| default_flags_for(basename));
                eprintln!("feff10-sys: found Intel compiler at {path}");
                return (path.to_string(), flags);
            }
        }
    }

    panic!(
        "feff10-sys: No Fortran compiler found. \
         Install gfortran, ifx, or flang-new, or set FEFF_FC env var."
    );
}

/// Return default flags for a given compiler.
fn default_flags_for(compiler: &str) -> String {
    let march = if env::var("FEFF_NO_NATIVE").is_ok() {
        "" // Allow disabling -march=native for cross-compilation
    } else {
        " -march=native"
    };

    // LTO: link-time optimization for cross-file inlining.
    // Disabled by default — benchmarks show gfortran LTO is ~8% slower
    // for FEFF10 due to increased code size and cache pressure in tight
    // ODE loops. Enable with FEFF_LTO=1 if your workload benefits.
    let lto = env::var("FEFF_LTO").is_ok();

    if compiler.contains("gfortran") {
        // -fallow-argument-mismatch needed for gfortran >= 10
        let lto_flag = if lto { " -flto=auto" } else { "" };
        format!("-ffree-line-length-none -cpp -O3 -fallow-argument-mismatch{march}{lto_flag}")
    } else if compiler.contains("ifx") {
        let lto_flag = if lto { " -ipo" } else { "" };
        // -no-vec: workaround for ifx 2025.3 ICE in VPlan vectorizer on
        // FF2X/ff2chijas.f90. Auto-vectorization has minimal impact on FEFF10
        // (scalar ODE code; vectorized math is in MKL). Scalar -O3 + -xHost
        // instruction selection is preserved.
        format!("-O3 -fpp -xHost -no-vec{lto_flag}")
    } else if compiler.contains("ifort") {
        let lto_flag = if lto { " -ipo" } else { "" };
        format!("-O3 -xHost{lto_flag}")
    } else if compiler.contains("flang") {
        // LLVM flang-new: free-form is default, -cpp enables preprocessing.
        // -fno-automatic: places local arrays in static storage instead of the stack.
        // Without this, flang-new stack-allocates ALL local arrays (including huge ones
        // like the 300MB arrays in paths.f90), causing stack overflow. gfortran avoids
        // this via -fmax-stack-var-size which auto-promotes large arrays to the heap.
        // FEFF10 is single-threaded and non-recursive, so static storage is safe.
        let lto_flag = if lto { " -flto" } else { "" };
        format!("-O3 -cpp -fno-automatic{march}{lto_flag}")
    } else {
        "-O3".to_string()
    }
}

/// Detect an optimized BLAS/LAPACK library.
/// Returns (ldflags, deptype) where deptype="_MKL" excludes the naive MATH/lu.f90.
/// Set FEFF_BLAS=none to disable.
fn detect_blas(compiler: &str) -> (String, String) {
    if env::var("FEFF_BLAS").as_deref() == Ok("none") {
        eprintln!("feff10-sys: BLAS disabled (FEFF_BLAS=none), using naive MATH/lu.f90");
        return (String::new(), String::new());
    }

    // 1. Honor explicit FEFF_BLAS setting
    if let Ok(blas) = env::var("FEFF_BLAS") {
        eprintln!("feff10-sys: using FEFF_BLAS={blas}");
        return (blas, "_MKL".to_string());
    }

    // 2. On macOS, use Accelerate framework (always available, optimized for Apple Silicon)
    if cfg!(target_os = "macos") {
        let framework_dir = Path::new("/System/Library/Frameworks/Accelerate.framework");
        if framework_dir.is_dir() {
            eprintln!("feff10-sys: using Apple Accelerate for BLAS/LAPACK");
            return ("-framework Accelerate".to_string(), "_MKL".to_string());
        }
    }

    // 3. Intel MKL — preferred on Linux for best LAPACK/BLAS performance.
    //    Auto-detects from MKLROOT env or standard oneAPI install paths.
    if cfg!(target_os = "linux") {
        if let Some(mkl_flags) = detect_mkl(compiler) {
            return (mkl_flags, "_MKL".to_string());
        }
    }

    // 4. OpenBLAS (Linux fallback)
    if cfg!(target_os = "linux") {
        // Try pkg-config first
        if let Ok(output) = Command::new("pkg-config").args(["--libs", "openblas"]).output() {
            if output.status.success() {
                let libs = String::from_utf8_lossy(&output.stdout).trim().to_string();
                eprintln!("feff10-sys: using OpenBLAS via pkg-config: {libs}");
                return (libs, "_MKL".to_string());
            }
        }
        // Try direct -lopenblas
        if Path::new("/usr/lib/x86_64-linux-gnu/libopenblas.so").exists()
            || Path::new("/usr/lib64/libopenblas.so").exists()
            || Path::new("/usr/lib/libopenblas.so").exists()
        {
            eprintln!("feff10-sys: using OpenBLAS (-lopenblas)");
            return ("-lopenblas".to_string(), "_MKL".to_string());
        }
        // Try system LAPACK/BLAS
        if Path::new("/usr/lib/x86_64-linux-gnu/liblapack.so").exists()
            || Path::new("/usr/lib64/liblapack.so").exists()
        {
            eprintln!("feff10-sys: using system LAPACK/BLAS");
            return ("-llapack -lblas".to_string(), "_MKL".to_string());
        }
    }

    eprintln!("feff10-sys: no optimized BLAS found, using naive MATH/lu.f90");
    (String::new(), String::new())
}

/// Auto-detect Intel MKL and return appropriate linker flags.
/// For ifx/ifort: uses -qmkl=sequential (compiler handles everything).
/// For gfortran/flang: uses explicit library linking.
fn detect_mkl(compiler: &str) -> Option<String> {
    let mkl_root = find_mkl_root()?;
    let lib_dir = mkl_root.join("lib/intel64");

    // Verify the key libraries exist
    if !lib_dir.join("libmkl_core.so").exists() && !lib_dir.join("libmkl_core.a").exists() {
        eprintln!("feff10-sys: MKL root found at {} but libraries missing", mkl_root.display());
        return None;
    }

    if compiler.contains("ifx") || compiler.contains("ifort") {
        // Intel compilers: use explicit static MKL linking to avoid runtime
        // LD_LIBRARY_PATH requirements. -qmkl only works reliably when setvars.sh
        // has been sourced; explicit paths are more robust.
        let flags = format!(
            "-L{lib} -Wl,--start-group {lib}/libmkl_intel_lp64.a {lib}/libmkl_sequential.a {lib}/libmkl_core.a -Wl,--end-group -lpthread -lm -ldl",
            lib = lib_dir.display()
        );
        eprintln!("feff10-sys: using MKL with Intel static linkage ({})", mkl_root.display());
        Some(flags)
    } else {
        // gfortran/flang: explicit static link flags.
        // Static linking avoids runtime dependency on MKL shared libraries.
        // Use --start-group/--end-group to resolve circular dependencies between MKL libs.
        let flags = format!(
            "-L{lib} -Wl,--start-group {lib}/libmkl_gf_lp64.a {lib}/libmkl_sequential.a {lib}/libmkl_core.a -Wl,--end-group -lpthread -lm -ldl",
            lib = lib_dir.display()
        );
        eprintln!("feff10-sys: using MKL with gfortran static linkage ({})", mkl_root.display());
        Some(flags)
    }
}

/// Find the MKL installation root directory.
fn find_mkl_root() -> Option<PathBuf> {
    // 1. MKLROOT environment variable (set by setvars.sh)
    if let Ok(root) = env::var("MKLROOT") {
        let path = PathBuf::from(&root);
        if path.is_dir() {
            return Some(path);
        }
    }

    // 2. Probe standard Intel oneAPI installation paths
    let candidates = [
        "/opt/intel/oneapi/mkl/latest",
    ];
    for path in &candidates {
        let p = PathBuf::from(path);
        if p.is_dir() {
            return Some(p);
        }
    }

    None
}

/// Return extra linker flags needed for a given compiler.
fn ldflags_for(compiler: &str) -> String {
    let mut flags = Vec::new();
    let lto = env::var("FEFF_LTO").is_ok();

    // LTO flags must also be passed to the linker
    if lto {
        if compiler.contains("gfortran") {
            flags.push("-flto=auto".to_string());
        } else if compiler.contains("flang") {
            flags.push("-flto".to_string());
        }
        // ifx/ifort: -ipo is handled by the compiler driver automatically
    }

    if compiler.contains("ifx") || compiler.contains("ifort") {
        // Statically link Intel runtime libs (libimf, libsvml, libirc, etc.)
        // so executables don't need LD_LIBRARY_PATH pointing to oneAPI.
        flags.push("-static-intel".to_string());
    }

    if compiler.contains("flang") {
        // LLVM flang-new needs libgcc for compiler-rt builtins (__divdc3, etc.)
        // on platforms where compiler-rt is not automatically linked.
        if let Ok(output) = Command::new("gcc").arg("-print-libgcc-file-name").output() {
            if output.status.success() {
                let libgcc_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if let Some(dir) = Path::new(&libgcc_path).parent() {
                    flags.push(format!("-L{} -lgcc", dir.display()));
                }
            }
        }

        // Safety net: increase default stack size on macOS (default 8MB).
        // The -fno-automatic flag (in FLAGS) handles the main issue by placing
        // local arrays in static storage, but this provides extra margin.
        if cfg!(target_os = "macos") {
            flags.push("-Wl,-stack_size,0x4000000".to_string());
        }
    }

    flags.join(" ")
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

        // Skip .git directories
        if entry.file_name() == ".git" || entry.file_name() == ".github" {
            continue;
        }
        // Skip doc, examples, Bugs directories (not needed for build)
        let name = entry.file_name();
        if name == "doc" || name == "examples" || name == "Bugs" || name == "windows" || name == "windowsNoMkl" || name == "project" {
            continue;
        }

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path);
        } else if ty.is_file() {
            fs::copy(&src_path, &dst_path).unwrap_or_else(|e| {
                panic!("Failed to copy {} -> {}: {e}", src_path.display(), dst_path.display());
            });
        }
    }
}
