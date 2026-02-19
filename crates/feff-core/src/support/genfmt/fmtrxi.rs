use super::m_genfmt::{FmatiMatrix, LambdaIndex, TensorError};
use num_complex::Complex64;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum FmtrxiError {
    #[error("lambda basis cannot be empty")]
    EmptyLambdaBasis,
    #[error(
        "lambda limits (lam1={lam1_limit}, lam2={lam2_limit}) exceed lambda basis size={lambda_count}"
    )]
    InvalidLimits {
        lam1_limit: usize,
        lam2_limit: usize,
        lambda_count: usize,
    },
    #[error(transparent)]
    Tensor(#[from] TensorError),
}

#[derive(Debug, Clone)]
pub struct FmtrxiInput<'a> {
    pub lambda: &'a [LambdaIndex],
    pub lam1_limit: usize,
    pub lam2_limit: usize,
    pub phase_shifts: &'a [Complex64],
    pub xnlm: &'a [Vec<f64>],
    pub clmi_left: &'a [Vec<Complex64>],
    pub clmi_right: &'a [Vec<Complex64>],
    pub eta: f64,
}

pub fn fmtrxi(input: FmtrxiInput<'_>) -> Result<FmatiMatrix, FmtrxiError> {
    if input.lambda.is_empty() {
        return Err(FmtrxiError::EmptyLambdaBasis);
    }
    if input.lam1_limit > input.lambda.len() || input.lam2_limit > input.lambda.len() {
        return Err(FmtrxiError::InvalidLimits {
            lam1_limit: input.lam1_limit,
            lam2_limit: input.lam2_limit,
            lambda_count: input.lambda.len(),
        });
    }

    let lam_count = input.lam1_limit.max(input.lam2_limit);
    let mut matrix = FmatiMatrix::zeroed(lam_count);
    if lam_count == 0 {
        return Ok(matrix);
    }

    let ilmax = input
        .phase_shifts
        .len()
        .min(input.xnlm.len())
        .min(input.clmi_left.len())
        .min(input.clmi_right.len());

    for lam1 in 0..input.lam1_limit {
        let lambda1 = input.lambda[lam1];
        for lam2 in 0..input.lam2_limit {
            let lambda2 = input.lambda[lam2];
            let ilmin = lambda1.m.abs().max(lambda2.m.abs()).max(1) as usize;
            let mut value = Complex64::new(0.0, 0.0);

            for il in ilmin..ilmax {
                let m1_abs = lambda1.m.unsigned_abs() as usize;
                let m2_abs = lambda2.m.unsigned_abs() as usize;

                let xnlm_1 = match input.xnlm[il].get(m1_abs).copied() {
                    Some(term) if term.abs() > f64::EPSILON => term,
                    _ => continue,
                };
                let xnlm_2 = match input.xnlm[il].get(m2_abs).copied() {
                    Some(term) if term.abs() > f64::EPSILON => term,
                    _ => continue,
                };

                let left_index = lambda1.n.saturating_add(m1_abs);
                let right_index = lambda2.n;
                let cl_left = input
                    .clmi_left
                    .get(il)
                    .and_then(|row| row.get(left_index))
                    .copied()
                    .unwrap_or_else(|| Complex64::new(0.0, 0.0));
                let cl_right = input
                    .clmi_right
                    .get(il)
                    .and_then(|row| row.get(right_index))
                    .copied()
                    .unwrap_or_else(|| Complex64::new(0.0, 0.0));

                let gam = cl_left * xnlm_1 * minus_one_to_power(lambda1.m);
                let gamtl = cl_right * ((2 * il + 1) as f64 / xnlm_2);
                value += gam * gamtl * input.phase_shifts[il];
            }

            let eta_phase = Complex64::new(0.0, -input.eta * lambda1.m as f64).exp();
            matrix.set(lam1, lam2, value * eta_phase)?;
        }
    }

    Ok(matrix)
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
    use super::{FmtrxiInput, fmtrxi};
    use crate::support::genfmt::m_genfmt::LambdaIndex;
    use num_complex::Complex64;

    fn simple_xnlm() -> Vec<Vec<f64>> {
        vec![
            vec![1.0, 1.0, 1.0],
            vec![1.0, 2.0, 2.5],
            vec![1.0, 3.0, 3.5],
            vec![1.0, 4.0, 4.5],
        ]
    }

    fn simple_clmi() -> Vec<Vec<Complex64>> {
        vec![
            vec![Complex64::new(0.0, 0.0); 4],
            vec![Complex64::new(1.0, 0.0); 4],
            vec![Complex64::new(0.75, 0.25); 4],
            vec![Complex64::new(0.5, -0.2); 4],
        ]
    }

    #[test]
    fn fmtrxi_builds_non_zero_scattering_matrix() {
        let lambda = vec![LambdaIndex { m: 0, n: 0 }, LambdaIndex { m: 1, n: 0 }];
        let matrix = fmtrxi(FmtrxiInput {
            lambda: &lambda,
            lam1_limit: 2,
            lam2_limit: 2,
            phase_shifts: &[
                Complex64::new(0.0, 0.0),
                Complex64::new(1.0, 0.0),
                Complex64::new(0.8, 0.1),
                Complex64::new(0.6, -0.2),
            ],
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.2,
        })
        .expect("valid input should produce matrix");

        let diagonal = matrix
            .get(0, 0)
            .expect("diagonal matrix entry should exist");
        let off_diagonal = matrix
            .get(1, 0)
            .expect("off-diagonal matrix entry should exist");
        assert!(diagonal.norm() > 0.0);
        assert!(off_diagonal.norm() > 0.0);
    }

    #[test]
    fn fmtrxi_eta_phase_changes_non_zero_m_rows() {
        let lambda = vec![LambdaIndex { m: 1, n: 0 }];
        let with_eta = fmtrxi(FmtrxiInput {
            lambda: &lambda,
            lam1_limit: 1,
            lam2_limit: 1,
            phase_shifts: &[Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0)],
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.4,
        })
        .expect("valid input should produce matrix");

        let without_eta = fmtrxi(FmtrxiInput {
            lambda: &lambda,
            lam1_limit: 1,
            lam2_limit: 1,
            phase_shifts: &[Complex64::new(0.0, 0.0), Complex64::new(1.0, 0.0)],
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.0,
        })
        .expect("valid input should produce matrix");

        assert_ne!(
            with_eta.get(0, 0).expect("matrix lookup should succeed"),
            without_eta.get(0, 0).expect("matrix lookup should succeed")
        );
    }
}
