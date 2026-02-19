const MAX_OSCILLATORS: usize = 13;
const FORM_FACTOR_POINTS: usize = 81;

#[derive(Debug, Clone)]
pub struct Fpf0Input<'a> {
    pub iz: f64,
    pub iholep: usize,
    pub srho: &'a [f64],
    pub dr: &'a [f64],
    pub hx: f64,
    pub dgc0: &'a [f64],
    pub dpc0: &'a [f64],
    pub dgc_orbitals: &'a [Vec<f64>],
    pub dpc_orbitals: &'a [Vec<f64>],
    pub eatom: f64,
    pub xnel: &'a [f64],
    pub norb: usize,
    pub eorb: &'a [f64],
    pub kappa: &'a [i32],
    pub alphfs: f64,
    pub bohr: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Oscillator {
    pub strength: f64,
    pub energy: f64,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormFactorPoint {
    pub q: f64,
    pub f0: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fpf0Output {
    pub energy_term: f64,
    pub fpcorr: f64,
    pub oscillators: Vec<Oscillator>,
    pub form_factors: Vec<FormFactorPoint>,
}

#[derive(Debug, Clone)]
pub struct SommInput<'a> {
    pub dr: &'a [f64],
    pub xpc: &'a [f64],
    pub xqc: &'a [f64],
    pub hx: f64,
    pub xirf: f64,
    pub mode: i32,
    pub np: usize,
}

pub type SommFn = dyn Fn(&SommInput<'_>) -> f64;

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum Fpf0Error {
    #[error(
        "norb={norb} exceeds available arrays (xnel={xnel}, eorb={eorb}, kappa={kappa}, dgc={dgc}, dpc={dpc})"
    )]
    InvalidOrbitalCount {
        norb: usize,
        xnel: usize,
        eorb: usize,
        kappa: usize,
        dgc: usize,
        dpc: usize,
    },
    #[error("iholep must be in 1..=norb, got {iholep}")]
    InvalidInitialHole { iholep: usize },
    #[error(
        "orbital {index} radial table length mismatch: need at least {need}, got dgc={dgc}, dpc={dpc}"
    )]
    OrbitalLengthMismatch {
        index: usize,
        need: usize,
        dgc: usize,
        dpc: usize,
    },
    #[error("initial-state kappa cannot be zero")]
    InvalidInitialKappa,
}

pub fn fpf0(input: &Fpf0Input<'_>, somm: &SommFn) -> Result<Fpf0Output, Fpf0Error> {
    if input.norb > input.xnel.len()
        || input.norb > input.eorb.len()
        || input.norb > input.kappa.len()
        || input.norb > input.dgc_orbitals.len()
        || input.norb > input.dpc_orbitals.len()
    {
        return Err(Fpf0Error::InvalidOrbitalCount {
            norb: input.norb,
            xnel: input.xnel.len(),
            eorb: input.eorb.len(),
            kappa: input.kappa.len(),
            dgc: input.dgc_orbitals.len(),
            dpc: input.dpc_orbitals.len(),
        });
    }

    if input.iholep == 0 || input.iholep > input.norb {
        return Err(Fpf0Error::InvalidInitialHole {
            iholep: input.iholep,
        });
    }

    let np = input
        .srho
        .len()
        .min(input.dr.len())
        .min(input.dgc0.len())
        .min(input.dpc0.len());

    let mut i = 0usize;
    while i < input.norb {
        if input.dgc_orbitals[i].len() < np || input.dpc_orbitals[i].len() < np {
            return Err(Fpf0Error::OrbitalLengthMismatch {
                index: i + 1,
                need: np,
                dgc: input.dgc_orbitals[i].len(),
                dpc: input.dpc_orbitals[i].len(),
            });
        }
        i += 1;
    }

    let ihole = input.iholep - 1;
    let kinit = input.kappa[ihole];
    if kinit == 0 {
        return Err(Fpf0Error::InvalidInitialKappa);
    }

    let fpcorr = -((input.iz / 82.5).powf(2.37));
    let energy_term = input.eatom * input.alphfs * input.alphfs * 5.0 / 3.0;

    let mut oscillators = Vec::with_capacity(MAX_OSCILLATORS);
    oscillators.push(Oscillator {
        strength: 2.0 * (kinit.abs() as f64),
        energy: input.eorb[ihole],
        index: input.iholep,
    });

    let mut xpc = vec![0.0_f64; np];
    let xqc = vec![0.0_f64; np];

    let mut iorb = 0usize;
    while iorb < input.norb && oscillators.len() < MAX_OSCILLATORS {
        if input.xnel[iorb] > 0.0 {
            let jkap = input.kappa[iorb];
            if jkap + kinit == 0 || (jkap - kinit).abs() == 1 {
                let mut kdif = jkap - kinit;
                if kdif.abs() > 1 {
                    kdif = 0;
                }
                let twoj = 2.0 * (kinit.abs() as f64) - 1.0;
                let (xmult1, xmult2) = dipole_multipliers(kdif, kinit, twoj);
                let xk0 = (input.eorb[iorb] - input.eorb[ihole]).abs() * input.alphfs;

                let mut ir = 0usize;
                while ir < np {
                    let xj0 = spherical_bessel_j0(xk0 * input.dr[ir]);
                    xpc[ir] = (xmult1 * input.dgc0[ir] * input.dpc_orbitals[iorb][ir]
                        + xmult2 * input.dpc0[ir] * input.dgc_orbitals[iorb][ir])
                        * xj0;
                    ir += 1;
                }

                let xirf = somm(&SommInput {
                    dr: &input.dr[..np],
                    xpc: &xpc,
                    xqc: &xqc,
                    hx: input.hx,
                    xirf: 2.0,
                    mode: 0,
                    np,
                });

                oscillators.push(Oscillator {
                    strength: xirf * xirf / 3.0,
                    energy: input.eorb[iorb],
                    index: iorb + 1,
                });
            }
        }
        iorb += 1;
    }

    let dq = 0.5 * input.bohr;
    let mut form_factors = Vec::with_capacity(FORM_FACTOR_POINTS);

    let mut iq = 1usize;
    while iq <= FORM_FACTOR_POINTS {
        let xk0 = dq * (iq - 1) as f64;

        let mut ir = 0usize;
        while ir < np {
            let mut xj0 = 1.0;
            if iq > 1 {
                xj0 = spherical_bessel_j0(xk0 * input.dr[ir]);
            }
            xpc[ir] = input.srho[ir] * input.dr[ir] * input.dr[ir] * xj0;
            ir += 1;
        }

        let xirf = somm(&SommInput {
            dr: &input.dr[..np],
            xpc: &xpc,
            xqc: &xqc,
            hx: input.hx,
            xirf: 2.0,
            mode: 0,
            np,
        });

        form_factors.push(FormFactorPoint {
            q: 0.5 * (iq - 1) as f64,
            f0: xirf,
        });
        iq += 1;
    }

    Ok(Fpf0Output {
        energy_term,
        fpcorr,
        oscillators,
        form_factors,
    })
}

fn spherical_bessel_j0(x: f64) -> f64 {
    if x.abs() <= 1.0e-14 { 1.0 } else { x.sin() / x }
}

fn dipole_multipliers(kdif: i32, kinit: i32, twoj: f64) -> (f64, f64) {
    if kdif == -1 && kinit > 0 {
        (0.0, (2.0 * (twoj + 1.0) * (twoj - 1.0) / twoj).sqrt())
    } else if kdif == -1 && kinit < 0 {
        (
            0.0,
            -(2.0 * (twoj + 1.0) * (twoj + 3.0) / (twoj + 2.0)).sqrt(),
        )
    } else if kdif == 0 && kinit > 0 {
        (
            -((twoj + 1.0) * twoj / (twoj + 2.0)).sqrt(),
            -((twoj + 1.0) * (twoj + 2.0) / twoj).sqrt(),
        )
    } else if kdif == 0 && kinit < 0 {
        (
            ((twoj + 1.0) * (twoj + 2.0) / twoj).sqrt(),
            ((twoj + 1.0) * twoj / (twoj + 2.0)).sqrt(),
        )
    } else if kdif == 1 && kinit > 0 {
        (
            (2.0 * (twoj + 1.0) * (twoj + 3.0) / (twoj + 2.0)).sqrt(),
            0.0,
        )
    } else if kdif == 1 && kinit < 0 {
        (-(2.0 * (twoj + 1.0) * (twoj - 1.0) / twoj).sqrt(), 0.0)
    } else {
        (0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{Fpf0Input, fpf0};

    fn somm_sum_xpc(input: &super::SommInput<'_>) -> f64 {
        input.xpc.iter().take(input.np).sum::<f64>()
    }

    #[test]
    fn fpf0_generates_oscillator_table_and_form_factor_grid() {
        let dgc_orb = vec![vec![1.0, 1.0, 1.0], vec![2.0, 2.0, 2.0]];
        let dpc_orb = vec![vec![0.5, 0.5, 0.5], vec![1.0, 1.0, 1.0]];

        let output = fpf0(
            &Fpf0Input {
                iz: 26.0,
                iholep: 1,
                srho: &[1.0, 1.0, 1.0],
                dr: &[1.0, 2.0, 3.0],
                hx: 1.0,
                dgc0: &[1.0, 1.0, 1.0],
                dpc0: &[0.5, 0.5, 0.5],
                dgc_orbitals: &dgc_orb,
                dpc_orbitals: &dpc_orb,
                eatom: 10.0,
                xnel: &[2.0, 1.0],
                norb: 2,
                eorb: &[5.0, 7.0],
                kappa: &[1, 2],
                alphfs: 0.5,
                bohr: 2.0,
            },
            &somm_sum_xpc,
        )
        .expect("fpf0 should succeed");

        assert_eq!(output.oscillators[0].index, 1);
        assert_eq!(output.oscillators[0].strength, 2.0);
        assert_eq!(output.oscillators.len(), 2);
        assert_eq!(output.form_factors.len(), 81);
        assert!((output.form_factors[0].f0 - 14.0).abs() <= 1.0e-12);
        assert!((output.form_factors[1].q - 0.5).abs() <= 1.0e-12);
    }

    #[test]
    fn fpf0_rejects_invalid_initial_hole_index() {
        let dgc_orb = vec![vec![1.0; 2]];
        let dpc_orb = vec![vec![1.0; 2]];

        let error = fpf0(
            &Fpf0Input {
                iz: 1.0,
                iholep: 0,
                srho: &[1.0, 1.0],
                dr: &[1.0, 2.0],
                hx: 1.0,
                dgc0: &[1.0, 1.0],
                dpc0: &[1.0, 1.0],
                dgc_orbitals: &dgc_orb,
                dpc_orbitals: &dpc_orb,
                eatom: 1.0,
                xnel: &[1.0],
                norb: 1,
                eorb: &[1.0],
                kappa: &[1],
                alphfs: 1.0,
                bohr: 1.0,
            },
            &somm_sum_xpc,
        )
        .expect_err("invalid iholep should fail");

        assert!(matches!(error, super::Fpf0Error::InvalidInitialHole { .. }));
    }
}
