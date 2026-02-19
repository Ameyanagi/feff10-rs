use super::m_genfmt::MAX_K_CHANNELS;
use num_complex::Complex64;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MmtrjasError {
    #[error("mu basis cannot be empty")]
    EmptyMuBasis,
    #[error("q-grid cannot be empty")]
    EmptyQGrid,
    #[error("q phase and beta vectors must have the same length")]
    MismatchedQGrid,
    #[error("channel count {0} exceeds MAX_K_CHANNELS={MAX_K_CHANNELS}")]
    TooManyChannels(usize),
}

#[derive(Debug, Clone)]
pub struct MmtrjasInput<'a> {
    pub mu_values: &'a [i32],
    pub lind: &'a [usize],
    pub q_phases: &'a [Complex64],
    pub q_beta: &'a [f64],
    pub eta_start: f64,
    pub eta_end: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JasSideMatrices {
    pub left: Vec<Vec<Vec<Complex64>>>,
    pub right: Vec<Vec<Vec<Complex64>>>,
}

impl JasSideMatrices {
    pub fn q_count(&self) -> usize {
        self.left.len()
    }
}

pub fn mmtrjas(input: MmtrjasInput<'_>) -> Result<JasSideMatrices, MmtrjasError> {
    if input.mu_values.is_empty() {
        return Err(MmtrjasError::EmptyMuBasis);
    }
    if input.q_phases.is_empty() {
        return Err(MmtrjasError::EmptyQGrid);
    }
    if input.q_phases.len() != input.q_beta.len() {
        return Err(MmtrjasError::MismatchedQGrid);
    }
    if input.lind.len() > MAX_K_CHANNELS {
        return Err(MmtrjasError::TooManyChannels(input.lind.len()));
    }

    let q_count = input.q_phases.len();
    let mu_count = input.mu_values.len();
    let channel_count = input.lind.len();

    let mut left = vec![vec![vec![Complex64::new(0.0, 0.0); channel_count]; mu_count]; q_count];
    let mut right = vec![vec![vec![Complex64::new(0.0, 0.0); channel_count]; mu_count]; q_count];

    for iq in 0..q_count {
        let phase = input.q_phases[iq];
        let beta = input.q_beta[iq];
        let beta_scale = 0.5 * (1.0 + beta.cos());

        for (mu_index, &mu) in input.mu_values.iter().enumerate() {
            let mu_phase = phase.powi(mu);
            let left_eta = Complex64::new(0.0, -input.eta_start * mu as f64).exp();
            let right_eta = Complex64::new(0.0, -input.eta_end * mu as f64).exp();

            for (k, &l) in input.lind.iter().enumerate() {
                let l_factor = 1.0 / (1.0 + l as f64);
                left[iq][mu_index][k] = mu_phase * left_eta * (beta_scale * (1.0 + 0.5 * l_factor));
                right[iq][mu_index][k] =
                    mu_phase.conj() * right_eta * (beta_scale * (1.0 + l_factor));
            }
        }
    }

    Ok(JasSideMatrices { left, right })
}

#[cfg(test)]
mod tests {
    use super::{MmtrjasError, MmtrjasInput, mmtrjas};
    use num_complex::Complex64;

    #[test]
    fn mmtrjas_builds_left_and_right_matrices() {
        let input = MmtrjasInput {
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            q_phases: &[Complex64::new(1.0, 0.0), Complex64::new(0.0, 1.0)],
            q_beta: &[0.0, 0.5],
            eta_start: 0.1,
            eta_end: 0.2,
        };

        let matrices = mmtrjas(input).expect("valid NRIXS matrix should build");
        assert_eq!(matrices.q_count(), 2);
        assert_eq!(matrices.left.len(), 2);
        assert_eq!(matrices.left[0].len(), 3);
        assert_eq!(matrices.left[0][0].len(), 2);
    }

    #[test]
    fn mmtrjas_applies_distinct_left_and_right_eta_phases() {
        let input = MmtrjasInput {
            mu_values: &[-1, 1],
            lind: &[1],
            q_phases: &[Complex64::new(0.8, 0.2)],
            q_beta: &[0.3],
            eta_start: 0.0,
            eta_end: 0.7,
        };

        let matrices = mmtrjas(input).expect("valid input should build");
        let left = matrices.left[0][1][0];
        let right = matrices.right[0][1][0];
        assert_ne!(left, right);
    }

    #[test]
    fn mmtrjas_rejects_mismatched_q_vectors() {
        let input = MmtrjasInput {
            mu_values: &[0],
            lind: &[0],
            q_phases: &[Complex64::new(1.0, 0.0)],
            q_beta: &[],
            eta_start: 0.0,
            eta_end: 0.0,
        };

        let error = mmtrjas(input).expect_err("mismatched q vectors should fail");
        assert_eq!(error, MmtrjasError::MismatchedQGrid);
    }
}
