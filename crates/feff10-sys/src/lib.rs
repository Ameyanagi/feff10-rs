//! Raw FFI bindings to the FEFF10 Fortran library.
//!
//! Each function corresponds to a FEFF10 pipeline stage. The Fortran subroutines
//! operate on files in the **current working directory** — the caller must `chdir`
//! to the desired working directory before calling any of these functions.
//!
//! # Safety
//!
//! All functions are `unsafe` because they:
//! - Call Fortran code that performs file I/O in the current working directory
//! - Are not thread-safe (Fortran module state is global)
//! - May call `stop` (process termination) on unrecoverable errors

unsafe extern "C" {
    pub fn feff_rdinp();
    pub fn feff_dmdw();
    pub fn feff_atomic();
    pub fn feff_pot();
    pub fn feff_ldos();
    pub fn feff_screen();
    pub fn feff_crpa();
    pub fn feff_opconsat();
    pub fn feff_xsph();
    pub fn feff_fms();
    pub fn feff_mkgtr();
    pub fn feff_path();
    pub fn feff_genfmt();
    pub fn feff_ff2x();
    pub fn feff_sfconv();
    pub fn feff_compton();
    pub fn feff_eels();
    pub fn feff_rhorrp();
}
