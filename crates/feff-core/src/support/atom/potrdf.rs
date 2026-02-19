use super::aprdev::aprdev;
use super::yzkrdf::{YzkrdfContext, YzkrdfError, YzkrdfInput, YzkrdfSource, yzkrdf};

pub type AkeatoFn = dyn FnMut(usize, usize, i32) -> f64;
pub type BkeatoFn = dyn FnMut(usize, usize, i32) -> f64;

#[derive(Debug, Clone)]
pub struct PotrdfInput<'a> {
    pub ia: usize,
    pub cg: &'a [Vec<f64>],
    pub cp: &'a [Vec<f64>],
    pub bg: &'a [Vec<f64>],
    pub bp: &'a [Vec<f64>],
    pub fl: &'a [f64],
    pub fix: &'a [f64],
    pub xnel: &'a [f64],
    pub kap: &'a [i32],
    pub nmax: &'a [usize],
    pub eps: &'a [f64],
    pub nre: &'a [i32],
    pub norb: usize,
    pub norbsc: usize,
    pub ndor: usize,
    pub idim: usize,
    pub method: i32,
    pub ipl: i32,
    pub cl: f64,
    pub dr: &'a [f64],
    pub dvn: &'a [f64],
    pub anoy: &'a [f64],
    pub hx: f64,
    pub nem: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PotrdfOutput {
    pub dv: Vec<f64>,
    pub av: Vec<f64>,
    pub eg: Vec<f64>,
    pub ep: Vec<f64>,
    pub ceg: Vec<f64>,
    pub cep: Vec<f64>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum PotrdfError {
    #[error("ia must be in 1..=norb, got ia={ia}, norb={norb}")]
    InvalidTargetOrbital { ia: usize, norb: usize },
    #[error("cl must be non-zero")]
    ZeroCl,
    #[error("xnel(ia) must be non-zero")]
    ZeroOccupation,
    #[error("input length mismatch for {name}: need at least {need}, got {got}")]
    LengthMismatch {
        name: &'static str,
        need: usize,
        got: usize,
    },
    #[error("orbital table row mismatch for {name}[{index}]: need at least {need}, got {got}")]
    RowLengthMismatch {
        name: &'static str,
        index: usize,
        need: usize,
        got: usize,
    },
    #[error("dr[{index}] must be > 0, got {value}")]
    NonPositiveRadius { index: usize, value: f64 },
    #[error(transparent)]
    Yzkrdf(#[from] YzkrdfError),
}

pub fn potrdf(
    input: &PotrdfInput<'_>,
    akeato: &mut AkeatoFn,
    bkeato: &mut BkeatoFn,
) -> Result<PotrdfOutput, PotrdfError> {
    validate_input(input)?;

    let ia_idx = input.ia - 1;
    let xnel_ia = input.xnel[ia_idx];
    if xnel_ia == 0.0 {
        return Err(PotrdfError::ZeroOccupation);
    }

    let yz_context = YzkrdfContext {
        cg: input.cg.to_vec(),
        cp: input.cp.to_vec(),
        bg: input.bg.to_vec(),
        bp: input.bp.to_vec(),
        fl: input.fl.to_vec(),
        nmax: input.nmax.to_vec(),
        dr: input.dr.to_vec(),
        hx: input.hx,
        ndor: input.ndor,
        idim: input.idim,
        nem: input.nem,
    };

    let mut at = vec![0.0_f64; input.idim];
    let mut bt = vec![0.0_f64; input.idim];
    let mut dv = vec![0.0_f64; input.idim];
    let mut eg = vec![0.0_f64; input.idim];
    let mut ep = vec![0.0_f64; input.idim];
    let mut av = vec![0.0_f64; input.ndor];
    let mut ceg = vec![0.0_f64; input.ndor];
    let mut cep = vec![0.0_f64; input.ndor];

    let mut i = 0usize;
    while i < input.ndor.min(input.anoy.len()) {
        av[i] = input.anoy[i];
        i += 1;
    }

    let jia = 2 * input.kap[ia_idx].abs() - 1;
    let mut k = 0_i32;

    while k < jia {
        let mut dg_seed = vec![0.0_f64; input.idim];
        let mut ag_seed = vec![0.0_f64; input.ndor];
        let mut max0 = 0usize;

        let mut j = 0usize;
        while j < input.norb {
            let m = 2 * input.kap[j].abs() - 1;
            if k > m {
                j += 1;
                continue;
            }

            let mut a = akeato(input.ia, j + 1, k) / xnel_ia;
            if a == 0.0 {
                j += 1;
                continue;
            }

            let max_j = input.nmax[j].min(input.idim);
            let mut ir = 0usize;
            while ir < max_j {
                dg_seed[ir] +=
                    a * (input.cg[j][ir] * input.cg[j][ir] + input.cp[j][ir] * input.cp[j][ir]);
                ir += 1;
            }

            let n = 2 * input.kap[j].abs() - k;
            let l = input.ndor as i32 + 2 - n;
            if l > 0 {
                a *= input.fix[j] * input.fix[j];
                let mut term = 1usize;
                while term <= l as usize {
                    let m_index = n - 2 + term as i32;
                    if m_index >= 1 && m_index <= input.ndor as i32 {
                        let idx = (m_index - 1) as usize;
                        ag_seed[idx] += a
                            * (aprdev(&input.bg[j], &input.bg[j], term)
                                + aprdev(&input.bp[j], &input.bp[j], term));
                    }
                    term += 1;
                }
            }

            if max_j > max0 {
                max0 = max_j;
            }

            j += 1;
        }

        let mut ap0 = 0.0_f64;
        let mut ag_out = vec![0.0_f64; input.ndor];

        if max0 >= 3 {
            let yz = yzkrdf(
                &YzkrdfInput {
                    source: YzkrdfSource::Prebuilt {
                        id: max0,
                        dg: &dg_seed,
                        ag: &ag_seed,
                    },
                    k,
                },
                &yz_context,
            )?;

            let mut coeff = 0usize;
            while coeff < input.ndor {
                ag_out[coeff] = yz.ag[coeff];
                coeff += 1;
            }

            let mut ir = 0usize;
            while ir < input.idim {
                dv[ir] += yz.dg[ir];
                ir += 1;
            }

            ap0 = yz.ap;
        }

        let mut coeff = 0usize;
        while coeff < input.ndor {
            let l = k + coeff as i32 + 4;
            if l <= input.ndor as i32 {
                av[(l - 1) as usize] -= ag_out[coeff];
            }
            coeff += 1;
        }

        k += 2;
        if k >= 1 && k <= input.ndor as i32 {
            av[(k - 1) as usize] += ap0;
        }
    }

    if input.method != 0 {
        let mut j = 0usize;
        while j < input.norb {
            if j == ia_idx {
                j += 1;
                continue;
            }

            let max0 = input.nmax[j].min(input.idim);
            let jj = 2 * input.kap[j].abs() - 1;
            let kma = (jj + jia) / 2;
            let mut k = (jj - kma).abs();
            if input.kap[j] * input.kap[ia_idx] < 0 {
                k += 1;
            }

            while k <= kma {
                let a = bkeato(j + 1, input.ia, k) / xnel_ia;
                if a != 0.0 {
                    let yz = yzkrdf(
                        &YzkrdfInput {
                            source: YzkrdfSource::Orbitals {
                                i: j + 1,
                                j: input.ia,
                            },
                            k,
                        },
                        &yz_context,
                    )?;

                    let mut ir = 0usize;
                    while ir < max0 {
                        eg[ir] += a * yz.dg[ir] * input.cg[j][ir];
                        ep[ir] += a * yz.dg[ir] * input.cp[j][ir];
                        ir += 1;
                    }

                    let n = k + 1 + input.kap[j].abs() - input.kap[ia_idx].abs();
                    if n <= input.ndor as i32 {
                        let mut coeff = n;
                        while coeff <= input.ndor as i32 {
                            let idx = (coeff - 1) as usize;
                            let src = (coeff + 1 - n) as usize;
                            if src >= 1 && src <= input.bg[j].len() {
                                let scale = a * yz.ap * input.fix[j] / input.fix[ia_idx];
                                ceg[idx] += input.bg[j][src - 1] * scale;
                                cep[idx] += input.bp[j][src - 1] * scale;
                            }
                            coeff += 1;
                        }
                    }

                    let i_start = 2 * input.kap[j].abs() + 1;
                    if i_start <= input.ndor as i32 {
                        let mut coeff = i_start;
                        while coeff <= input.ndor as i32 {
                            let idx = (coeff - 1) as usize;
                            let order = (coeff + 1 - i_start) as usize;
                            let scale = a * input.fix[j] * input.fix[j];
                            ceg[idx] -= scale * aprdev(&yz.ag, &input.bg[j], order);
                            cep[idx] -= scale * aprdev(&yz.ag, &input.bp[j], order);
                            coeff += 1;
                        }
                    }
                }

                k += 2;
            }

            j += 1;
        }
    }

    if input.ipl != 0 {
        let mut j = 0usize;
        while j < input.norbsc.min(input.norb) {
            if input.kap[j] != input.kap[ia_idx] || j == ia_idx {
                j += 1;
                continue;
            }
            if input.nre[j] < 0 && input.nre[ia_idx] < 0 {
                j += 1;
                continue;
            }

            let m = input.ia.max(j + 1);
            let i_index = input.ia.min(j + 1) + ((m - 1) * (m - 2)) / 2;
            if i_index == 0 || i_index > input.eps.len() {
                j += 1;
                continue;
            }

            let a = input.eps[i_index - 1] * input.xnel[j];
            let max0 = input.nmax[j].min(input.idim);

            let mut ir = 0usize;
            while ir < max0 {
                at[ir] += a * input.cg[j][ir];
                bt[ir] += a * input.cp[j][ir];
                ir += 1;
            }

            let mut coeff = 0usize;
            while coeff < input.ndor {
                ceg[coeff] += input.bg[j][coeff] * a;
                cep[coeff] += input.bp[j][coeff] * a;
                coeff += 1;
            }

            j += 1;
        }
    }

    let mut coeff = 0usize;
    while coeff < input.ndor {
        av[coeff] /= input.cl;
        ceg[coeff] /= input.cl;
        cep[coeff] /= input.cl;
        coeff += 1;
    }

    let mut ir = 0usize;
    while ir < input.idim {
        dv[ir] = (dv[ir] / input.dr[ir] + input.dvn[ir]) / input.cl;
        eg[ir] = (eg[ir] + at[ir] * input.dr[ir]) / input.cl;
        ep[ir] = (ep[ir] + bt[ir] * input.dr[ir]) / input.cl;
        ir += 1;
    }

    Ok(PotrdfOutput {
        dv,
        av,
        eg,
        ep,
        ceg,
        cep,
    })
}

fn validate_input(input: &PotrdfInput<'_>) -> Result<(), PotrdfError> {
    if input.cl == 0.0 {
        return Err(PotrdfError::ZeroCl);
    }
    if input.ia == 0 || input.ia > input.norb {
        return Err(PotrdfError::InvalidTargetOrbital {
            ia: input.ia,
            norb: input.norb,
        });
    }

    ensure_len("cg", input.cg.len(), input.norb)?;
    ensure_len("cp", input.cp.len(), input.norb)?;
    ensure_len("bg", input.bg.len(), input.norb)?;
    ensure_len("bp", input.bp.len(), input.norb)?;
    ensure_len("fl", input.fl.len(), input.norb)?;
    ensure_len("fix", input.fix.len(), input.norb)?;
    ensure_len("xnel", input.xnel.len(), input.norb)?;
    ensure_len("kap", input.kap.len(), input.norb)?;
    ensure_len("nmax", input.nmax.len(), input.norb)?;
    ensure_len("nre", input.nre.len(), input.norb)?;
    ensure_len("dr", input.dr.len(), input.idim)?;
    ensure_len("dvn", input.dvn.len(), input.idim)?;
    ensure_len("anoy", input.anoy.len(), input.ndor)?;

    let mut i = 0usize;
    while i < input.idim {
        if input.dr[i] <= 0.0 {
            return Err(PotrdfError::NonPositiveRadius {
                index: i,
                value: input.dr[i],
            });
        }
        i += 1;
    }

    let mut orb = 0usize;
    while orb < input.norb {
        ensure_row_len("cg", orb + 1, input.cg[orb].len(), input.idim)?;
        ensure_row_len("cp", orb + 1, input.cp[orb].len(), input.idim)?;
        ensure_row_len("bg", orb + 1, input.bg[orb].len(), input.ndor)?;
        ensure_row_len("bp", orb + 1, input.bp[orb].len(), input.ndor)?;
        orb += 1;
    }

    Ok(())
}

fn ensure_len(name: &'static str, got: usize, need: usize) -> Result<(), PotrdfError> {
    if got < need {
        return Err(PotrdfError::LengthMismatch { name, need, got });
    }
    Ok(())
}

fn ensure_row_len(
    name: &'static str,
    index: usize,
    got: usize,
    need: usize,
) -> Result<(), PotrdfError> {
    if got < need {
        return Err(PotrdfError::RowLengthMismatch {
            name,
            index,
            need,
            got,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PotrdfInput, potrdf};

    #[test]
    fn potrdf_reduces_to_nuclear_potential_when_akeato_is_zero() {
        let cg = vec![vec![1.0, 0.8, 0.6, 0.4, 0.2]];
        let cp = vec![vec![0.0, 0.0, 0.0, 0.0, 0.0]];
        let bg = vec![vec![1.0, 0.0, 0.0, 0.0]];
        let bp = vec![vec![0.0, 0.0, 0.0, 0.0]];
        let fl = vec![1.0];
        let fix = vec![1.0];
        let xnel = vec![2.0];
        let kap = vec![1];
        let nmax = vec![5];
        let eps = vec![0.0];
        let nre = vec![0];
        let dr = vec![0.5, 0.7, 1.0, 1.4, 2.0];
        let dvn = vec![0.1; 5];
        let anoy = vec![0.2, 0.3, 0.4, 0.5];

        let input = PotrdfInput {
            ia: 1,
            cg: &cg,
            cp: &cp,
            bg: &bg,
            bp: &bp,
            fl: &fl,
            fix: &fix,
            xnel: &xnel,
            kap: &kap,
            nmax: &nmax,
            eps: &eps,
            nre: &nre,
            norb: 1,
            norbsc: 1,
            ndor: 4,
            idim: 5,
            method: 0,
            ipl: 0,
            cl: 2.0,
            dr: &dr,
            dvn: &dvn,
            anoy: &anoy,
            hx: 0.05,
            nem: 0,
        };

        let output =
            potrdf(&input, &mut |_, _, _| 0.0, &mut |_, _, _| 0.0).expect("potrdf should succeed");

        assert!((output.dv[0] - 0.05).abs() <= 1.0e-12);
        assert!((output.av[0] - 0.1).abs() <= 1.0e-12);
        assert!(output.eg.iter().all(|value| value.abs() <= 1.0e-12));
        assert!(output.ep.iter().all(|value| value.abs() <= 1.0e-12));
    }

    #[test]
    fn potrdf_populates_exchange_terms_when_method_is_enabled() {
        let cg = vec![vec![1.0, 0.8, 0.6, 0.4, 0.2], vec![0.9, 0.7, 0.5, 0.3, 0.1]];
        let cp = vec![vec![0.1, 0.1, 0.1, 0.1, 0.1], vec![0.2, 0.2, 0.2, 0.2, 0.2]];
        let bg = vec![vec![1.0, 0.0, 0.0, 0.0], vec![0.5, 0.0, 0.0, 0.0]];
        let bp = vec![vec![0.2, 0.0, 0.0, 0.0], vec![0.1, 0.0, 0.0, 0.0]];
        let fl = vec![1.0, 1.0];
        let fix = vec![1.0, 1.0];
        let xnel = vec![2.0, 1.0];
        let kap = vec![1, 1];
        let nmax = vec![5, 5];
        let eps = vec![0.0, 0.1, 0.0];
        let nre = vec![0, 0];
        let dr = vec![0.5, 0.7, 1.0, 1.4, 2.0];
        let dvn = vec![0.1; 5];
        let anoy = vec![0.2, 0.3, 0.4, 0.5];

        let input = PotrdfInput {
            ia: 1,
            cg: &cg,
            cp: &cp,
            bg: &bg,
            bp: &bp,
            fl: &fl,
            fix: &fix,
            xnel: &xnel,
            kap: &kap,
            nmax: &nmax,
            eps: &eps,
            nre: &nre,
            norb: 2,
            norbsc: 2,
            ndor: 4,
            idim: 5,
            method: 1,
            ipl: 1,
            cl: 2.0,
            dr: &dr,
            dvn: &dvn,
            anoy: &anoy,
            hx: 0.05,
            nem: 0,
        };

        let output =
            potrdf(&input, &mut |_, _, _| 0.2, &mut |_, _, _| 0.3).expect("potrdf should succeed");

        assert!(output.eg.iter().any(|value| value.abs() > 1.0e-9));
        assert!(output.ep.iter().any(|value| value.abs() > 1.0e-9));
        assert!(output.ceg.iter().any(|value| value.abs() > 1.0e-9));
        assert!(output.cep.iter().any(|value| value.abs() > 1.0e-9));
    }
}
