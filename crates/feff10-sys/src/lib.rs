include!(concat!(env!("OUT_DIR"), "/paths.rs"));

use std::path::{Path, PathBuf};

/// Path to the directory containing compiled FEFF executables.
pub fn bin_dir() -> &'static Path {
    Path::new(FEFF_BIN_DIR)
}

/// Path to a specific FEFF executable by name.
pub fn executable(name: &str) -> PathBuf {
    bin_dir().join(name)
}

/// All FEFF executables in canonical pipeline order.
pub const PIPELINE_EXECUTABLES: &[&str] = &[
    "rdinp", "dmdw", "atomic", "pot", "ldos", "screen", "crpa", "opconsat", "xsph", "fms",
    "mkgtr", "path", "genfmt", "ff2x", "sfconv", "compton", "eels", "rhorrp",
];
