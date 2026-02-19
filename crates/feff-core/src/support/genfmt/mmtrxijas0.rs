use super::m_genfmt::LambdaIndex;
use super::mmtrjas0::HbmatrsTensor;
use num_complex::Complex64;

pub type SpinLamTensor = Vec<Vec<Vec<Vec<Complex64>>>>;
pub type SpinLDecTensor = Vec<Vec<Vec<Vec<Vec<Complex64>>>>>;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum Mmtrxijas0Error {
    #[error("lambda basis cannot be empty")]
    EmptyLambdaBasis,
    #[error("lam_limit {lam_limit} exceeds lambda basis size={lambda_count}")]
    InvalidLimit {
        lam_limit: usize,
        lambda_count: usize,
    },
    #[error("q-grid cannot be empty")]
    EmptyQGrid,
    #[error("q-grid dimensions are inconsistent")]
    MismatchedQGrid,
    #[error("channel dimensions are inconsistent")]
    MismatchedChannelCount,
    #[error("jinit must be non-negative")]
    InvalidJinit,
    #[error(transparent)]
    Tensor(#[from] super::m_genfmt::TensorError),
}

#[derive(Debug, Clone)]
pub struct Mmtrxijas0Input<'a> {
    pub lambda: &'a [LambdaIndex],
    pub lam_limit: usize,
    pub lind: &'a [usize],
    pub rkk: &'a [Vec<Complex64>],
    pub q_weights: &'a [f64],
    pub hbmatrs: &'a HbmatrsTensor,
    pub xnlm: &'a [Vec<f64>],
    pub clmi_left: &'a [Vec<Complex64>],
    pub clmi_right: &'a [Vec<Complex64>],
    pub eta: f64,
    pub jinit: i32,
    pub ldecmx: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mmtrxijas0Output {
    pub mj_values: Vec<i32>,
    pub fmats: SpinLamTensor,
    pub lgfmats: SpinLDecTensor,
}

pub fn mmtrxijas0(input: &Mmtrxijas0Input<'_>) -> Result<Mmtrxijas0Output, Mmtrxijas0Error> {
    if input.lambda.is_empty() {
        return Err(Mmtrxijas0Error::EmptyLambdaBasis);
    }
    if input.lam_limit > input.lambda.len() {
        return Err(Mmtrxijas0Error::InvalidLimit {
            lam_limit: input.lam_limit,
            lambda_count: input.lambda.len(),
        });
    }
    if input.q_weights.is_empty() {
        return Err(Mmtrxijas0Error::EmptyQGrid);
    }
    if input.jinit < 0 {
        return Err(Mmtrxijas0Error::InvalidJinit);
    }

    let q_count = input.q_weights.len();
    let channel_count = input.lind.len();
    if channel_count != input.hbmatrs.channel_count() {
        return Err(Mmtrxijas0Error::MismatchedChannelCount);
    }
    if input.rkk.len() != q_count {
        return Err(Mmtrxijas0Error::MismatchedQGrid);
    }
    if input.rkk.iter().any(|row| row.len() != channel_count) {
        return Err(Mmtrxijas0Error::MismatchedChannelCount);
    }

    let mut qsum = vec![Complex64::new(0.0, 0.0); channel_count];
    for q in 0..q_count {
        for (k, qsum_entry) in qsum.iter_mut().enumerate().take(channel_count) {
            *qsum_entry += input.rkk[q][k] * input.rkk[q][k] * input.q_weights[q];
        }
    }

    let mj_values: Vec<i32> = (-input.jinit..=input.jinit).step_by(2).collect();
    let mj_count = mj_values.len();
    let lam_limit = input.lam_limit;
    let ldecs = input.ldecmx + 1;

    let zero = Complex64::new(0.0, 0.0);
    let mut fmats = vec![vec![vec![vec![zero; lam_limit]; lam_limit]; 2]; mj_count];
    let mut lgfmats = vec![vec![vec![vec![vec![zero; lam_limit]; lam_limit]; ldecs]; 2]; mj_count];

    for lam1 in 0..lam_limit {
        let lambda1 = input.lambda[lam1];

        for lam2 in 0..lam_limit {
            let lambda2 = input.lambda[lam2];
            let mut spin_sum = [zero, zero];
            let mut spin_lsum = vec![[zero, zero]; ldecs];

            for (k, (&l, qsum_value)) in input.lind.iter().zip(qsum.iter()).enumerate() {
                let m1_abs = lambda1.m.unsigned_abs() as usize;
                let m2_abs = lambda2.m.unsigned_abs() as usize;
                if m1_abs > l || m2_abs > l {
                    continue;
                }

                let gamma_l = gamma_left(l, lambda1, input.xnlm, input.clmi_left);
                let gamma_r = gamma_right(l, lambda2, input.xnlm, input.clmi_right);
                if gamma_l == zero && gamma_r == zero {
                    continue;
                }

                let hb = input.hbmatrs.get(lambda2.m, lambda1.m, k)?;
                let base = *qsum_value * hb * gamma_r * gamma_l / (2 * l + 1) as f64;
                let spin_base = [base, base.conj()];

                for spin in 0..2 {
                    spin_sum[spin] += spin_base[spin];
                }
                if l <= input.ldecmx {
                    for spin in 0..2 {
                        spin_lsum[l][spin] += spin_base[spin];
                    }
                }
            }

            let eta_phase = Complex64::new(0.0, -input.eta * lambda1.m as f64).exp();
            for (mj_index, mj) in mj_values.iter().enumerate() {
                let mj_phase = Complex64::new(0.0, *mj as f64 * 0.25).exp();
                for spin in 0..2 {
                    let spin_phase = if spin == 0 { mj_phase } else { mj_phase.conj() };
                    fmats[mj_index][spin][lam2][lam1] = spin_sum[spin] * eta_phase * spin_phase;

                    for l in 0..ldecs {
                        lgfmats[mj_index][spin][l][lam2][lam1] =
                            spin_lsum[l][spin] * eta_phase * spin_phase;
                    }
                }
            }
        }
    }

    Ok(Mmtrxijas0Output {
        mj_values,
        fmats,
        lgfmats,
    })
}

fn gamma_left(
    l: usize,
    lambda: LambdaIndex,
    xnlm: &[Vec<f64>],
    clmi_left: &[Vec<Complex64>],
) -> Complex64 {
    let m_abs = lambda.m.unsigned_abs() as usize;
    let x = match xnlm.get(l).and_then(|row| row.get(m_abs)).copied() {
        Some(value) if value.abs() > f64::EPSILON => value,
        _ => return Complex64::new(0.0, 0.0),
    };

    let coeff_index = lambda.n.saturating_add(m_abs);
    let coeff = clmi_left
        .get(l)
        .and_then(|row| row.get(coeff_index))
        .copied()
        .unwrap_or_else(|| Complex64::new(0.0, 0.0));

    coeff * x * minus_one_to_power(lambda.m)
}

fn gamma_right(
    l: usize,
    lambda: LambdaIndex,
    xnlm: &[Vec<f64>],
    clmi_right: &[Vec<Complex64>],
) -> Complex64 {
    let m_abs = lambda.m.unsigned_abs() as usize;
    let x = match xnlm.get(l).and_then(|row| row.get(m_abs)).copied() {
        Some(value) if value.abs() > f64::EPSILON => value,
        _ => return Complex64::new(0.0, 0.0),
    };

    let coeff = clmi_right
        .get(l)
        .and_then(|row| row.get(lambda.n))
        .copied()
        .unwrap_or_else(|| Complex64::new(0.0, 0.0));

    coeff * ((2 * l + 1) as f64 / x)
}

fn minus_one_to_power(exponent: i32) -> f64 {
    if exponent.rem_euclid(2) == 0 {
        1.0
    } else {
        -1.0
    }
}

#[cfg(test)]
mod tests {
    use super::{Mmtrxijas0Input, mmtrxijas0};
    use crate::support::genfmt::m_genfmt::LambdaIndex;
    use crate::support::genfmt::mmtrjas0::{Mmtrjas0Input, mmtrjas0};
    use num_complex::Complex64;

    fn simple_xnlm() -> Vec<Vec<f64>> {
        vec![
            vec![1.0, 1.0],
            vec![1.0, 2.0],
            vec![1.0, 3.0],
            vec![1.0, 4.0],
        ]
    }

    fn simple_clmi() -> Vec<Vec<Complex64>> {
        vec![
            vec![Complex64::new(1.0, 0.0); 3],
            vec![Complex64::new(0.8, 0.1); 3],
            vec![Complex64::new(0.6, -0.1); 3],
            vec![Complex64::new(0.4, 0.2); 3],
        ]
    }

    #[test]
    fn mmtrxijas0_builds_spin_resolved_output() {
        let hb = mmtrjas0(Mmtrjas0Input {
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            eta_start: 0.1,
            eta_end: 0.2,
        })
        .expect("valid hb matrix input should build");

        let lambda = vec![LambdaIndex { m: 0, n: 0 }, LambdaIndex { m: 1, n: 0 }];
        let output = mmtrxijas0(&Mmtrxijas0Input {
            lambda: &lambda,
            lam_limit: 2,
            lind: &[0, 1],
            rkk: &[
                vec![Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.1)],
                vec![Complex64::new(0.7, -0.2), Complex64::new(0.4, 0.2)],
            ],
            q_weights: &[1.0, 0.5],
            hbmatrs: &hb,
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.3,
            jinit: 1,
            ldecmx: 1,
        })
        .expect("valid input should produce spin-resolved matrix output");

        assert_eq!(output.mj_values, vec![-1, 1]);
        assert!(output.fmats[0][0][0][0].norm() > 0.0);
        assert!(output.fmats[1][1][1][1].norm() > 0.0);
    }

    #[test]
    fn mmtrxijas0_scales_with_q_weights() {
        let hb = mmtrjas0(Mmtrjas0Input {
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            eta_start: 0.0,
            eta_end: 0.0,
        })
        .expect("valid hb matrix input should build");

        let lambda = vec![LambdaIndex { m: 0, n: 0 }];
        let strong = mmtrxijas0(&Mmtrxijas0Input {
            lambda: &lambda,
            lam_limit: 1,
            lind: &[0, 1],
            rkk: &[vec![Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.0)]],
            q_weights: &[1.0],
            hbmatrs: &hb,
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.0,
            jinit: 1,
            ldecmx: 1,
        })
        .expect("valid input should produce output");

        let weak = mmtrxijas0(&Mmtrxijas0Input {
            lambda: &lambda,
            lam_limit: 1,
            lind: &[0, 1],
            rkk: &[vec![Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.0)]],
            q_weights: &[0.2],
            hbmatrs: &hb,
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.0,
            jinit: 1,
            ldecmx: 1,
        })
        .expect("valid input should produce output");

        assert!(strong.fmats[0][0][0][0].norm() > weak.fmats[0][0][0][0].norm());
    }
}
