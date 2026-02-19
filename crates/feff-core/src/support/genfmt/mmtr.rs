use super::m_genfmt::{BmatiTensor, MAX_K_CHANNELS, TensorError};
use num_complex::Complex64;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MmtrError {
    #[error("mu basis cannot be empty")]
    EmptyMuBasis,
    #[error("lind and bmat_diagonal must have the same length")]
    MismatchedChannelInputs,
    #[error("channel count {0} exceeds MAX_K_CHANNELS={MAX_K_CHANNELS}")]
    TooManyChannels(usize),
    #[error(transparent)]
    Tensor(#[from] TensorError),
}

#[derive(Debug, Clone)]
pub struct MmtrInput<'a> {
    pub mu_values: &'a [i32],
    pub lind: &'a [usize],
    pub bmat_diagonal: &'a [Complex64],
    pub eta_start: f64,
    pub eta_end: f64,
    pub polarized: bool,
}

pub fn mmtr(input: MmtrInput<'_>) -> Result<BmatiTensor, MmtrError> {
    if input.mu_values.is_empty() {
        return Err(MmtrError::EmptyMuBasis);
    }
    if input.lind.len() != input.bmat_diagonal.len() {
        return Err(MmtrError::MismatchedChannelInputs);
    }
    if input.lind.len() > MAX_K_CHANNELS {
        return Err(MmtrError::TooManyChannels(input.lind.len()));
    }

    let channel_count = input.lind.len();
    let mut tensor = BmatiTensor::zeroed(input.mu_values.to_vec(), channel_count);

    for &mu1 in input.mu_values {
        for &mu2 in input.mu_values {
            if input.polarized {
                for k1 in 0..channel_count {
                    for k2 in 0..channel_count {
                        let l1 = input.lind[k1] as f64;
                        let l2 = input.lind[k2] as f64;
                        let angular = 1.0 / (1.0 + (mu1 - mu2).abs() as f64 + (l1 - l2).abs());
                        let phase = Complex64::new(
                            0.0,
                            -(input.eta_end * mu2 as f64 + input.eta_start * mu1 as f64),
                        )
                        .exp();
                        let value = input.bmat_diagonal[k1]
                            * input.bmat_diagonal[k2].conj()
                            * phase
                            * angular;
                        tensor.add(mu1, k1, mu2, k2, value)?;
                    }
                }
            } else {
                for k in 0..channel_count {
                    let l = input.lind[k] as f64;
                    let angular = 1.0 / (1.0 + (mu1 - mu2).abs() as f64 + l);
                    let value = input.bmat_diagonal[k] * angular;
                    tensor.add(mu1, k, mu2, k, value)?;
                }
            }
        }
    }

    Ok(tensor)
}

#[cfg(test)]
mod tests {
    use super::{MmtrError, MmtrInput, mmtr};
    use num_complex::Complex64;

    #[test]
    fn polarized_mmtr_populates_cross_channel_entries() {
        let input = MmtrInput {
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            bmat_diagonal: &[Complex64::new(1.0, 0.0), Complex64::new(0.5, 0.25)],
            eta_start: 0.1,
            eta_end: 0.2,
            polarized: true,
        };

        let matrix = mmtr(input).expect("polarized matrix should build");
        let value = matrix
            .get(-1, 0, 1, 1)
            .expect("cross-channel value should exist");
        assert!(value.norm() > 0.0);
    }

    #[test]
    fn unpolarized_mmtr_keeps_channel_diagonal() {
        let input = MmtrInput {
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            bmat_diagonal: &[Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
            eta_start: 0.0,
            eta_end: 0.0,
            polarized: false,
        };

        let matrix = mmtr(input).expect("unpolarized matrix should build");
        let diagonal = matrix
            .get(0, 1, 0, 1)
            .expect("channel-diagonal value should be present");
        let off_diagonal = matrix
            .get(0, 0, 0, 1)
            .expect("channel-off-diagonal value should be present");

        assert!(diagonal.norm() > 0.0);
        assert_eq!(off_diagonal, Complex64::new(0.0, 0.0));
    }

    #[test]
    fn mmtr_rejects_empty_mu_basis() {
        let input = MmtrInput {
            mu_values: &[],
            lind: &[0],
            bmat_diagonal: &[Complex64::new(1.0, 0.0)],
            eta_start: 0.0,
            eta_end: 0.0,
            polarized: true,
        };
        let error = mmtr(input).expect_err("empty mu basis should fail");
        assert_eq!(error, MmtrError::EmptyMuBasis);
    }
}
