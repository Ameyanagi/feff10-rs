/**
 * feff10.h — C interface to FEFF10 (X-ray absorption spectroscopy)
 *
 * Link with: libfeff10.a + Fortran runtime + BLAS/LAPACK
 *
 * Usage:
 *   1. Write a feff.inp file in the working directory
 *   2. chdir() to that directory
 *   3. Call stages in pipeline order (or a subset via CONTROL flags)
 *
 * IMPORTANT:
 *   - All functions use the current working directory for file I/O
 *   - Fortran global state is NOT thread-safe — run in a forked process
 *     or ensure single-threaded access
 *   - Each stage reads/writes intermediate files consumed by later stages
 *
 * Pipeline order:
 *   rdinp → dmdw → atomic → pot → ldos → screen → crpa → opconsat →
 *   xsph → fms → mkgtr → path → genfmt → ff2x → sfconv → compton →
 *   eels → rhorrp
 *
 * Linker flags (platform-dependent):
 *   Linux (ifx+MKL):  -lfeff10 -lifcore -limf -lsvml -lirc -lmkl_intel_lp64
 *                      -lmkl_sequential -lmkl_core -lpthread -lm -ldl
 *   Linux (gfortran):  -lfeff10 -lgfortran -lopenblas -lpthread -lm
 *   macOS (gfortran):  -lfeff10 -lgfortran -framework Accelerate
 *   Windows (gfortran): -lfeff10 -lgfortran -lpthread -lm
 */

#ifndef FEFF10_H
#define FEFF10_H

#ifdef __cplusplus
extern "C" {
#endif

/* Read and parse feff.inp */
void feff_rdinp(void);

/* Debye-Waller / DMDW calculation */
void feff_dmdw(void);

/* Atomic potentials */
void feff_atomic(void);

/* Self-consistent potentials */
void feff_pot(void);

/* Local density of states */
void feff_ldos(void);

/* Screening */
void feff_screen(void);

/* Constrained RPA */
void feff_crpa(void);

/* Optical constants */
void feff_opconsat(void);

/* Scattering phase shifts */
void feff_xsph(void);

/* Full multiple scattering */
void feff_fms(void);

/* Green's function transfer matrix */
void feff_mkgtr(void);

/* Path enumeration */
void feff_path(void);

/* Scattering amplitude generation */
void feff_genfmt(void);

/* Chi(k) and mu(E) output */
void feff_ff2x(void);

/* Spectral function convolution */
void feff_sfconv(void);

/* Compton scattering */
void feff_compton(void);

/* Electron energy loss spectroscopy */
void feff_eels(void);

/* Charge density / rho(r,r') */
void feff_rhorrp(void);

#ifdef __cplusplus
}
#endif

#endif /* FEFF10_H */
