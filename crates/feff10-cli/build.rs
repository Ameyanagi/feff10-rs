fn main() {
    // Forward feff10-sys build metadata to compile-time env vars
    if let Ok(fc) = std::env::var("DEP_FEFF10_FC") {
        println!("cargo:rustc-env=FEFF10_FC={fc}");
    }
    if let Ok(fflags) = std::env::var("DEP_FEFF10_FFLAGS") {
        println!("cargo:rustc-env=FEFF10_FFLAGS={fflags}");
    }
    if let Ok(blas) = std::env::var("DEP_FEFF10_BLAS") {
        println!("cargo:rustc-env=FEFF10_BLAS={blas}");
    }
    if let Ok(commit) = std::env::var("DEP_FEFF10_FEFF10_COMMIT") {
        println!("cargo:rustc-env=FEFF10_COMMIT={commit}");
    }
}
