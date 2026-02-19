use super::m_genfmt::LambdaIndex;
use super::mmtrjas::JasSideMatrices;
use num_complex::Complex64;

pub type MjQLamTensor = Vec<Vec<Vec<Complex64>>>;
pub type MjQLLamTensor = Vec<Vec<Vec<Vec<Complex64>>>>;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MmtrxijasError {
    #[error("lambda basis cannot be empty")]
    EmptyLambdaBasis,
    #[error("lam_limit {lam_limit} exceeds lambda basis size={lambda_count}")]
    InvalidLimit {
        lam_limit: usize,
        lambda_count: usize,
    },
    #[error("jinit must be non-negative")]
    InvalidJinit,
    #[error("q-grid cannot be empty")]
    EmptyQGrid,
    #[error("q-grid dimensions are inconsistent")]
    MismatchedQGrid,
    #[error("channel dimensions are inconsistent")]
    MismatchedChannelCount,
    #[error("mu basis does not contain m={0}")]
    MissingMu(i32),
}

#[derive(Debug, Clone)]
pub struct MmtrxijasInput<'a> {
    pub lambda: &'a [LambdaIndex],
    pub lam_limit: usize,
    pub mu_values: &'a [i32],
    pub lind: &'a [usize],
    pub rkk: &'a [Vec<Complex64>],
    pub q_weights: &'a [f64],
    pub side_matrices: &'a JasSideMatrices,
    pub xnlm: &'a [Vec<f64>],
    pub clmi_left: &'a [Vec<Complex64>],
    pub clmi_right: &'a [Vec<Complex64>],
    pub eta: f64,
    pub jinit: i32,
    pub ldecmx: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MmtrxijasOutput {
    pub mj_values: Vec<i32>,
    pub left: MjQLamTensor,
    pub right: MjQLamTensor,
    pub l_decomposed_left: MjQLLamTensor,
    pub l_decomposed_right: MjQLLamTensor,
}

pub fn mmtrxijas(input: &MmtrxijasInput<'_>) -> Result<MmtrxijasOutput, MmtrxijasError> {
    if input.lambda.is_empty() {
        return Err(MmtrxijasError::EmptyLambdaBasis);
    }
    if input.lam_limit > input.lambda.len() {
        return Err(MmtrxijasError::InvalidLimit {
            lam_limit: input.lam_limit,
            lambda_count: input.lambda.len(),
        });
    }
    if input.jinit < 0 {
        return Err(MmtrxijasError::InvalidJinit);
    }
    if input.q_weights.is_empty() {
        return Err(MmtrxijasError::EmptyQGrid);
    }

    let q_count = input.q_weights.len();
    let channel_count = input.lind.len();
    let mu_count = input.mu_values.len();

    if input.rkk.len() != q_count
        || input.side_matrices.left.len() != q_count
        || input.side_matrices.right.len() != q_count
    {
        return Err(MmtrxijasError::MismatchedQGrid);
    }

    for q in 0..q_count {
        if input.rkk[q].len() != channel_count {
            return Err(MmtrxijasError::MismatchedChannelCount);
        }
        if input.side_matrices.left[q].len() != mu_count
            || input.side_matrices.right[q].len() != mu_count
        {
            return Err(MmtrxijasError::MismatchedChannelCount);
        }

        for mu in 0..mu_count {
            if input.side_matrices.left[q][mu].len() != channel_count
                || input.side_matrices.right[q][mu].len() != channel_count
            {
                return Err(MmtrxijasError::MismatchedChannelCount);
            }
        }
    }

    let mj_values: Vec<i32> = (-input.jinit..=input.jinit).step_by(2).collect();
    let mj_count = mj_values.len();
    let lam_limit = input.lam_limit;
    let zero = Complex64::new(0.0, 0.0);

    let mut left = vec![vec![vec![zero; lam_limit]; q_count]; mj_count];
    let mut right = vec![vec![vec![zero; lam_limit]; q_count]; mj_count];

    let mut l_decomposed_left =
        vec![vec![vec![vec![zero; lam_limit]; input.ldecmx + 1]; q_count]; mj_count];
    let mut l_decomposed_right =
        vec![vec![vec![vec![zero; lam_limit]; input.ldecmx + 1]; q_count]; mj_count];

    for lam_index in 0..lam_limit {
        let lambda = input.lambda[lam_index];
        let mu_index = input
            .mu_values
            .iter()
            .position(|candidate| *candidate == lambda.m)
            .ok_or(MmtrxijasError::MissingMu(lambda.m))?;

        let phase = Complex64::new(0.0, -input.eta * lambda.m as f64).exp();

        for q in 0..q_count {
            let mut q_left = zero;
            let mut q_right = zero;
            let mut l_left = vec![zero; input.ldecmx + 1];
            let mut l_right = vec![zero; input.ldecmx + 1];

            for k in 0..channel_count {
                let l = input.lind[k];
                let gamma_l = gamma_left(l, lambda, input.xnlm, input.clmi_left);
                let gamma_r = gamma_right(l, lambda, input.xnlm, input.clmi_right);
                if gamma_l == zero && gamma_r == zero {
                    continue;
                }

                let weighted_rkk = input.rkk[q][k] * input.q_weights[q];
                let left_term = weighted_rkk * input.side_matrices.left[q][mu_index][k] * gamma_l;
                let right_term = weighted_rkk * input.side_matrices.right[q][mu_index][k] * gamma_r;

                q_left += left_term;
                q_right += right_term;

                if l <= input.ldecmx {
                    l_left[l] += left_term;
                    l_right[l] += right_term;
                }
            }

            for (mj_index, mj) in mj_values.iter().enumerate() {
                let mj_phase = Complex64::new(0.0, *mj as f64 * 0.5).exp();
                left[mj_index][q][lam_index] = q_left * phase * mj_phase;
                right[mj_index][q][lam_index] = q_right * phase * mj_phase.conj();

                for l in 0..=input.ldecmx {
                    l_decomposed_left[mj_index][q][l][lam_index] = l_left[l] * phase * mj_phase;
                    l_decomposed_right[mj_index][q][l][lam_index] =
                        l_right[l] * phase * mj_phase.conj();
                }
            }
        }
    }

    Ok(MmtrxijasOutput {
        mj_values,
        left,
        right,
        l_decomposed_left,
        l_decomposed_right,
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
    use super::{MmtrxijasError, MmtrxijasInput, mmtrxijas};
    use crate::support::genfmt::m_genfmt::LambdaIndex;
    use crate::support::genfmt::mmtrjas::{MmtrjasInput, mmtrjas};
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

    fn side_matrices() -> crate::support::genfmt::mmtrjas::JasSideMatrices {
        mmtrjas(MmtrjasInput {
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            q_phases: &[Complex64::new(1.0, 0.0), Complex64::new(0.0, 1.0)],
            q_beta: &[0.0, 0.4],
            eta_start: 0.1,
            eta_end: 0.2,
        })
        .expect("valid q-matrix input should build")
    }

    fn side_matrices_single_q() -> crate::support::genfmt::mmtrjas::JasSideMatrices {
        mmtrjas(MmtrjasInput {
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            q_phases: &[Complex64::new(1.0, 0.0)],
            q_beta: &[0.0],
            eta_start: 0.1,
            eta_end: 0.2,
        })
        .expect("valid single-q matrix input should build")
    }

    #[test]
    fn mmtrxijas_builds_weighted_left_and_right_tensors() {
        let lambda = vec![LambdaIndex { m: 0, n: 0 }, LambdaIndex { m: 1, n: 0 }];
        let output = mmtrxijas(&MmtrxijasInput {
            lambda: &lambda,
            lam_limit: 2,
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            rkk: &[
                vec![Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.2)],
                vec![Complex64::new(0.7, -0.1), Complex64::new(0.4, 0.1)],
            ],
            q_weights: &[1.0, 0.5],
            side_matrices: &side_matrices(),
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.3,
            jinit: 1,
            ldecmx: 1,
        })
        .expect("valid input should build output tensors");

        assert_eq!(output.mj_values, vec![-1, 1]);
        assert_eq!(output.left.len(), 2);
        assert_eq!(output.left[0].len(), 2);
        assert!(output.left[0][0][0].norm() > 0.0);
        assert!(output.right[1][1][1].norm() > 0.0);
    }

    #[test]
    fn mmtrxijas_scales_with_q_weights() {
        let lambda = vec![LambdaIndex { m: 0, n: 0 }];
        let base = mmtrxijas(&MmtrxijasInput {
            lambda: &lambda,
            lam_limit: 1,
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            rkk: &[
                vec![Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.0)],
                vec![Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.0)],
            ],
            q_weights: &[1.0, 1.0],
            side_matrices: &side_matrices(),
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.0,
            jinit: 1,
            ldecmx: 1,
        })
        .expect("valid input should build output tensors");

        let reduced = mmtrxijas(&MmtrxijasInput {
            lambda: &lambda,
            lam_limit: 1,
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            rkk: &[
                vec![Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.0)],
                vec![Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.0)],
            ],
            q_weights: &[0.2, 0.2],
            side_matrices: &side_matrices(),
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.0,
            jinit: 1,
            ldecmx: 1,
        })
        .expect("valid input should build output tensors");

        assert!(base.left[0][0][0].norm() > reduced.left[0][0][0].norm());
    }

    #[test]
    fn mmtrxijas_rejects_missing_mu_basis_member() {
        let lambda = vec![LambdaIndex { m: 2, n: 0 }];
        let error = mmtrxijas(&MmtrxijasInput {
            lambda: &lambda,
            lam_limit: 1,
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            rkk: &[vec![Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.0)]],
            q_weights: &[1.0],
            side_matrices: &side_matrices_single_q(),
            xnlm: &simple_xnlm(),
            clmi_left: &simple_clmi(),
            clmi_right: &simple_clmi(),
            eta: 0.0,
            jinit: 1,
            ldecmx: 1,
        })
        .expect_err("unknown mu should fail");

        assert_eq!(error, MmtrxijasError::MissingMu(2));
    }
}
