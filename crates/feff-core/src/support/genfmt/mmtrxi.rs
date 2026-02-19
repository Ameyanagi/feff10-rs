use super::m_genfmt::{BmatiTensor, FmatiMatrix, LambdaIndex, TensorError};
use num_complex::Complex64;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MmtrxiError {
    #[error("lambda basis cannot be empty")]
    EmptyLambdaBasis,
    #[error("lam_limit {lam_limit} exceeds lambda basis size={lambda_count}")]
    InvalidLimit {
        lam_limit: usize,
        lambda_count: usize,
    },
    #[error("rkk length {actual} must match channel count {expected} derived from lind and bmati")]
    RkkLengthMismatch { expected: usize, actual: usize },
    #[error("lind length {actual} must match bmati channel count {expected}")]
    LindLengthMismatch { expected: usize, actual: usize },
    #[error(transparent)]
    Tensor(#[from] TensorError),
}

#[derive(Debug, Clone)]
pub struct MmtrxiInput<'a> {
    pub lambda: &'a [LambdaIndex],
    pub lam_limit: usize,
    pub lind: &'a [usize],
    pub bmati: &'a BmatiTensor,
    pub rkk: &'a [Complex64],
    pub xnlm: &'a [Vec<f64>],
    pub clmi_left: &'a [Vec<Complex64>],
    pub clmi_right: &'a [Vec<Complex64>],
    pub eta: f64,
}

pub fn mmtrxi(input: MmtrxiInput<'_>) -> Result<FmatiMatrix, MmtrxiError> {
    if input.lambda.is_empty() {
        return Err(MmtrxiError::EmptyLambdaBasis);
    }
    if input.lam_limit > input.lambda.len() {
        return Err(MmtrxiError::InvalidLimit {
            lam_limit: input.lam_limit,
            lambda_count: input.lambda.len(),
        });
    }

    let channel_count = input.bmati.channel_count();
    if input.lind.len() != channel_count {
        return Err(MmtrxiError::LindLengthMismatch {
            expected: channel_count,
            actual: input.lind.len(),
        });
    }
    if input.rkk.len() != channel_count {
        return Err(MmtrxiError::RkkLengthMismatch {
            expected: channel_count,
            actual: input.rkk.len(),
        });
    }

    let mut matrix = FmatiMatrix::zeroed(input.lam_limit);

    for lam1 in 0..input.lam_limit {
        let lambda1 = input.lambda[lam1];
        for lam2 in 0..input.lam_limit {
            let lambda2 = input.lambda[lam2];
            let m1_abs = lambda1.m.unsigned_abs() as usize;
            let m2_abs = lambda2.m.unsigned_abs() as usize;
            let mut value = Complex64::new(0.0, 0.0);

            for k1 in 0..channel_count {
                for k2 in 0..channel_count {
                    let l1 = input.lind[k1];
                    let l2 = input.lind[k2];
                    if m1_abs > l1 || m2_abs > l2 {
                        continue;
                    }

                    let gam = gamma_left(l1, lambda1, input.xnlm, input.clmi_left);
                    let gamtl = gamma_right(l2, lambda2, input.xnlm, input.clmi_right);
                    if gam == Complex64::new(0.0, 0.0) || gamtl == Complex64::new(0.0, 0.0) {
                        continue;
                    }

                    let bm = input.bmati.get(lambda1.m, k1, lambda2.m, k2)?;
                    value += bm * input.rkk[k1] * input.rkk[k2] * gam * gamtl;
                }
            }

            let eta_phase = Complex64::new(0.0, -input.eta * lambda1.m as f64).exp();
            matrix.set(lam1, lam2, value * eta_phase)?;
        }
    }

    Ok(matrix)
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

    let index = lambda.n.saturating_add(m_abs);
    let coeff = clmi_left
        .get(l)
        .and_then(|row| row.get(index))
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
    use super::{MmtrxiError, MmtrxiInput, mmtrxi};
    use crate::support::genfmt::m_genfmt::{BmatiTensor, LambdaIndex};
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
            vec![Complex64::new(0.6, 0.2); 3],
            vec![Complex64::new(0.4, -0.1); 3],
        ]
    }

    fn build_bmati() -> BmatiTensor {
        let mut matrix = BmatiTensor::zeroed(vec![-1, 0, 1], 2);
        matrix
            .set(0, 0, 0, 0, Complex64::new(1.0, 0.0))
            .expect("set should succeed");
        matrix
            .set(1, 1, 0, 0, Complex64::new(0.5, 0.2))
            .expect("set should succeed");
        matrix
            .set(1, 1, 1, 1, Complex64::new(0.7, -0.1))
            .expect("set should succeed");
        matrix
    }

    #[test]
    fn mmtrxi_combines_bmati_and_rkk_into_fmati() {
        let lambda = vec![LambdaIndex { m: 0, n: 0 }, LambdaIndex { m: 1, n: 0 }];
        let matrix = mmtrxi(MmtrxiInput {
            lambda: &lambda,
            lam_limit: 2,
            lind: &[0, 1],
            bmati: &build_bmati(),
            rkk: &[Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.25)],
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.2,
        })
        .expect("valid input should produce matrix");

        let diagonal = matrix.get(0, 0).expect("diagonal should exist");
        let off_diagonal = matrix.get(1, 0).expect("off-diagonal should exist");
        assert!(diagonal.norm() > 0.0);
        assert!(off_diagonal.norm() > 0.0);
    }

    #[test]
    fn mmtrxi_rejects_rkk_length_mismatch() {
        let lambda = vec![LambdaIndex { m: 0, n: 0 }];
        let error = mmtrxi(MmtrxiInput {
            lambda: &lambda,
            lam_limit: 1,
            lind: &[0, 1],
            bmati: &build_bmati(),
            rkk: &[Complex64::new(1.0, 0.0)],
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.0,
        })
        .expect_err("rkk length mismatch should fail");

        assert_eq!(
            error,
            MmtrxiError::RkkLengthMismatch {
                expected: 2,
                actual: 1
            }
        );
    }
}
