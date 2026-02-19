use super::dentfa::dentfa;
use super::nucdev::{NucdevError, NucdevInput, nucdev};
use super::soldir::{SoldirError, SoldirInput, SoldirOutput, soldir};

pub const CL_ATOMIC_UNITS: f64 = 1.370_373e2;
pub const HX_DEFAULT: f64 = 5.0e-2;

#[derive(Debug, Clone)]
pub struct WfirdfInput<'a> {
    pub nz: f64,
    pub ch: f64,
    pub nq: &'a [i32],
    pub kap: &'a [i32],
    pub nmax: &'a [usize],
    pub norb: usize,
    pub ido: i32,
    pub idim: usize,
    pub ndor: usize,
    pub ibgp: usize,
    pub nuc: i32,
    pub testy: f64,
    pub rap: [f64; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct WfirdfOutput {
    pub en: Vec<f64>,
    pub cg: Vec<Vec<f64>>,
    pub cp: Vec<Vec<f64>>,
    pub bg: Vec<Vec<f64>>,
    pub bp: Vec<Vec<f64>>,
    pub fl: Vec<f64>,
    pub fix: Vec<f64>,
    pub dr: Vec<f64>,
    pub dvn: Vec<f64>,
    pub dv: Vec<f64>,
    pub av: Vec<f64>,
    pub anoy: Vec<f64>,
    pub nuc: i32,
    pub hx: f64,
    pub cl: f64,
    pub soldir_failures: Vec<usize>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum WfirdfError {
    #[error("norb must be >= 1")]
    InvalidNorb,
    #[error("input length mismatch for {name}: need at least {need}, got {got}")]
    LengthMismatch {
        name: &'static str,
        need: usize,
        got: usize,
    },
    #[error("nq at orbital {index} must be > 0, got {value}")]
    InvalidPrincipalQuantumNumber { index: usize, value: i32 },
    #[error("kap at orbital {index} must be non-zero")]
    InvalidKappa { index: usize },
    #[error("rap(1) must be non-zero")]
    InvalidRap,
    #[error("nuc index {0} is outside radial grid")]
    InvalidNuclearIndex(i32),
    #[error(transparent)]
    Nucdev(#[from] NucdevError),
    #[error(transparent)]
    Soldir(#[from] SoldirError),
}

pub fn wfirdf(input: &WfirdfInput<'_>) -> Result<WfirdfOutput, WfirdfError> {
    wfirdf_with(input, |soldir_input| {
        soldir(soldir_input).map_err(WfirdfError::Soldir)
    })
}

pub fn wfirdf_with<F>(
    input: &WfirdfInput<'_>,
    mut soldir_fn: F,
) -> Result<WfirdfOutput, WfirdfError>
where
    F: FnMut(&SoldirInput<'_>) -> Result<SoldirOutput, WfirdfError>,
{
    validate_input(input)?;

    let cl = CL_ATOMIC_UNITS;
    let dz = input.nz;
    let hx = HX_DEFAULT;
    let mut dr1 = input.nz * (-8.8_f64).exp();

    let mut nuc = if input.nuc <= 0 { 11 } else { input.nuc };
    let nuc_mode = if input.ido < 0 { -(nuc.max(5)) } else { nuc };

    let nucdev_output = nucdev(NucdevInput {
        dz,
        hx,
        nuc: nuc_mode,
        np: input.idim,
        ndor: input.ndor,
        dr1,
    })?;

    dr1 = nucdev_output.dr1;
    let dr = nucdev_output.dr;
    let dvn = nucdev_output.dv;
    let mut anoy = vec![0.0_f64; input.ibgp];
    let mut coeff = 0usize;
    while coeff < input.ibgp.min(nucdev_output.av.len()) {
        anoy[coeff] = nucdev_output.av[coeff];
        coeff += 1;
    }
    nuc = nucdev_output.nuc;

    if nuc <= 0 || nuc as usize > input.idim {
        return Err(WfirdfError::InvalidNuclearIndex(nuc));
    }

    let mut fl = vec![0.0_f64; input.norb];
    let mut fix = vec![0.0_f64; input.norb];

    let mut a = (dz / cl) * (dz / cl);
    if nuc > 1 {
        a = 0.0;
    }

    let mut j = 0usize;
    while j < input.norb {
        let b = input.kap[j] as f64 * input.kap[j] as f64 - a;
        fl[j] = b.sqrt();
        fix[j] = dr1.powf(fl[j] - input.kap[j].abs() as f64);
        j += 1;
    }

    let mut dv = vec![0.0_f64; input.idim];
    let mut i = 0usize;
    while i < input.idim {
        dv[i] = (dentfa(dr[i], dz, input.ch) + dvn[i]) / cl;
        i += 1;
    }

    let mut av = vec![0.0_f64; input.ibgp];
    let mut k = 0usize;
    while k < input.ibgp {
        av[k] = anoy[k] / cl;
        k += 1;
    }
    if input.ibgp > 1 {
        let nuc_index = (nuc - 1) as usize;
        av[1] += dentfa(dr[nuc_index], dz, input.ch) / cl;
    }

    let mut en = vec![0.0_f64; input.norb];
    let mut cg = vec![vec![0.0_f64; input.idim]; input.norb];
    let mut cp = vec![vec![0.0_f64; input.idim]; input.norb];
    let mut bg = vec![vec![0.0_f64; input.ibgp]; input.norb];
    let mut bp = vec![vec![0.0_f64; input.ibgp]; input.norb];

    let test1 = input.testy / input.rap[0];
    let mut soldir_failures = Vec::new();

    let mut orbital = 0usize;
    while orbital < input.norb {
        bg[orbital][0] = 1.0;

        let mut parity = input.nq[orbital] - input.kap[orbital].abs();
        if input.kap[orbital] < 0 {
            parity -= 1;
        }
        if parity % 2 == 0 {
            bg[orbital][0] = -bg[orbital][0];
        }

        if input.kap[orbital] > 0 {
            bp[orbital][0] = bg[orbital][0] * cl * (input.kap[orbital] as f64 + fl[orbital]) / dz;
            if nuc > 1 {
                bg[orbital][0] = 0.0;
            }
        } else {
            bp[orbital][0] = bg[orbital][0] * dz / (cl * (input.kap[orbital] as f64 - fl[orbital]));
            if nuc > 1 {
                bp[orbital][0] = 0.0;
            }
        }

        let nqf = input.nq[orbital] as f64;
        en[orbital] = -dz * dz / (nqf * nqf);

        let soldir_output = soldir_fn(&SoldirInput {
            en: en[orbital],
            fl: fl[orbital],
            agi: bg[orbital][0],
            api: bp[orbital][0],
            ainf: test1,
            nq: input.nq[orbital],
            kap: input.kap[orbital],
            max0: input.nmax[orbital].min(input.idim).max(1),
            method: 0,
            cl,
            dv: &dv,
            av: &av,
            dr: &dr,
            hx,
            test1,
            test2: test1,
            ndor: input.ndor,
            np: input.idim,
            nes: 50,
        })?;

        en[orbital] = soldir_output.en;

        if soldir_output.ifail {
            soldir_failures.push(orbital + 1);
        }

        let mut n = 0usize;
        while n < input.ibgp.min(soldir_output.ag.len()) {
            bg[orbital][n] = soldir_output.ag[n];
            bp[orbital][n] = soldir_output.ap[n];
            n += 1;
        }

        let mut m = 0usize;
        while m < input.idim.min(soldir_output.gg.len()) {
            cg[orbital][m] = soldir_output.gg[m];
            cp[orbital][m] = soldir_output.gp[m];
            m += 1;
        }

        orbital += 1;
    }

    Ok(WfirdfOutput {
        en,
        cg,
        cp,
        bg,
        bp,
        fl,
        fix,
        dr,
        dvn,
        dv,
        av,
        anoy,
        nuc,
        hx,
        cl,
        soldir_failures,
    })
}

fn validate_input(input: &WfirdfInput<'_>) -> Result<(), WfirdfError> {
    if input.norb == 0 {
        return Err(WfirdfError::InvalidNorb);
    }
    if input.rap[0] == 0.0 {
        return Err(WfirdfError::InvalidRap);
    }

    ensure_len("nq", input.nq.len(), input.norb)?;
    ensure_len("kap", input.kap.len(), input.norb)?;
    ensure_len("nmax", input.nmax.len(), input.norb)?;

    let mut i = 0usize;
    while i < input.norb {
        if input.nq[i] <= 0 {
            return Err(WfirdfError::InvalidPrincipalQuantumNumber {
                index: i + 1,
                value: input.nq[i],
            });
        }
        if input.kap[i] == 0 {
            return Err(WfirdfError::InvalidKappa { index: i + 1 });
        }
        i += 1;
    }

    Ok(())
}

fn ensure_len(name: &'static str, got: usize, need: usize) -> Result<(), WfirdfError> {
    if got < need {
        return Err(WfirdfError::LengthMismatch { name, need, got });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{WfirdfInput, wfirdf_with};
    use crate::support::atom::soldir::{SoldirInput, SoldirOutput};

    #[test]
    fn wfirdf_initializes_orbital_tables_with_mock_soldir() {
        let mut calls = 0usize;

        let output = wfirdf_with(
            &WfirdfInput {
                nz: 8.0,
                ch: 0.0,
                nq: &[1, 2],
                kap: &[1, -1],
                nmax: &[11, 11],
                norb: 2,
                ido: 1,
                idim: 11,
                ndor: 5,
                ibgp: 5,
                nuc: 1,
                testy: 1.0e-5,
                rap: [100.0, 10.0],
            },
            |input: &SoldirInput<'_>| {
                calls += 1;
                let mut gg = vec![0.0_f64; input.np];
                let mut gp = vec![0.0_f64; input.np];
                let mut i = 0usize;
                while i < input.np {
                    gg[i] = input.en.abs() / (i as f64 + 1.0);
                    gp[i] = gg[i] * 0.1;
                    i += 1;
                }

                Ok(SoldirOutput {
                    en: input.en * 0.9,
                    gg,
                    gp,
                    ag: vec![1.0; input.ndor],
                    ap: vec![0.5; input.ndor],
                    mat: 5,
                    max0: input.max0,
                    method: 1,
                    ifail: false,
                })
            },
        )
        .expect("wfirdf should succeed with mocked soldir");

        assert_eq!(calls, 2);
        assert_eq!(output.cg.len(), 2);
        assert_eq!(output.bg[0][0], 1.0);
        assert!(output.cp[1][0] > 0.0);
        assert!(output.en[0] < 0.0);
    }

    #[test]
    fn wfirdf_marks_failed_soldir_calls() {
        let output = wfirdf_with(
            &WfirdfInput {
                nz: 6.0,
                ch: 0.0,
                nq: &[1],
                kap: &[1],
                nmax: &[9],
                norb: 1,
                ido: -1,
                idim: 9,
                ndor: 5,
                ibgp: 5,
                nuc: 3,
                testy: 1.0e-5,
                rap: [100.0, 10.0],
            },
            |input: &SoldirInput<'_>| {
                Ok(SoldirOutput {
                    en: input.en,
                    gg: vec![0.1; input.np],
                    gp: vec![0.01; input.np],
                    ag: vec![0.2; input.ndor],
                    ap: vec![0.1; input.ndor],
                    mat: 3,
                    max0: input.max0,
                    method: 1,
                    ifail: true,
                })
            },
        )
        .expect("wfirdf should succeed even when soldir flags ifail");

        assert_eq!(output.soldir_failures, vec![1]);
        assert!(output.nuc >= 1);
    }
}
