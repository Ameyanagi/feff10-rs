use super::m_genfmt::{MAX_K_CHANNELS, TensorError};
use num_complex::Complex64;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum Mmtrjas0Error {
    #[error("mu basis cannot be empty")]
    EmptyMuBasis,
    #[error("channel count {0} exceeds MAX_K_CHANNELS={MAX_K_CHANNELS}")]
    TooManyChannels(usize),
}

#[derive(Debug, Clone)]
pub struct Mmtrjas0Input<'a> {
    pub mu_values: &'a [i32],
    pub lind: &'a [usize],
    pub eta_start: f64,
    pub eta_end: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HbmatrsTensor {
    mu_values: Vec<i32>,
    channel_count: usize,
    data: Vec<Complex64>,
}

impl HbmatrsTensor {
    pub fn get(&self, mu2: i32, mu1: i32, k: usize) -> Result<Complex64, TensorError> {
        let mu2_index = self.mu_index(mu2)?;
        let mu1_index = self.mu_index(mu1)?;
        let index = self.flat_index(mu2_index, mu1_index, k)?;
        Ok(self.data[index])
    }

    pub fn mu_values(&self) -> &[i32] {
        &self.mu_values
    }

    pub fn channel_count(&self) -> usize {
        self.channel_count
    }

    fn mu_index(&self, mu: i32) -> Result<usize, TensorError> {
        self.mu_values
            .iter()
            .position(|candidate| *candidate == mu)
            .ok_or(TensorError::UnknownMu { mu })
    }

    fn flat_index(
        &self,
        mu2_index: usize,
        mu1_index: usize,
        k: usize,
    ) -> Result<usize, TensorError> {
        if k >= self.channel_count {
            return Err(TensorError::InvalidChannel {
                channel: k,
                channel_count: self.channel_count,
            });
        }
        let mu_count = self.mu_values.len();
        Ok((mu2_index * mu_count + mu1_index) * self.channel_count + k)
    }
}

pub fn mmtrjas0(input: Mmtrjas0Input<'_>) -> Result<HbmatrsTensor, Mmtrjas0Error> {
    if input.mu_values.is_empty() {
        return Err(Mmtrjas0Error::EmptyMuBasis);
    }
    if input.lind.len() > MAX_K_CHANNELS {
        return Err(Mmtrjas0Error::TooManyChannels(input.lind.len()));
    }

    let mu_count = input.mu_values.len();
    let channel_count = input.lind.len();
    let mut data = vec![Complex64::new(0.0, 0.0); mu_count * mu_count * channel_count];

    for (mu2_index, &mu2) in input.mu_values.iter().enumerate() {
        for (mu1_index, &mu1) in input.mu_values.iter().enumerate() {
            for (k, &l) in input.lind.iter().enumerate() {
                let phase = Complex64::new(
                    0.0,
                    -(input.eta_start * mu1 as f64 + input.eta_end * mu2 as f64),
                )
                .exp();
                let angular = 1.0 / (1.0 + mu1.abs() as f64 + mu2.abs() as f64 + l as f64);
                let index = (mu2_index * mu_count + mu1_index) * channel_count + k;
                data[index] = phase * angular;
            }
        }
    }

    Ok(HbmatrsTensor {
        mu_values: input.mu_values.to_vec(),
        channel_count,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::{Mmtrjas0Input, mmtrjas0};
    use crate::support::genfmt::m_genfmt::TensorError;

    #[test]
    fn mmtrjas0_builds_expected_tensor_shape() {
        let tensor = mmtrjas0(Mmtrjas0Input {
            mu_values: &[-1, 0, 1],
            lind: &[0, 1],
            eta_start: 0.1,
            eta_end: 0.3,
        })
        .expect("valid input should build tensor");

        assert_eq!(tensor.mu_values(), &[-1, 0, 1]);
        assert_eq!(tensor.channel_count(), 2);

        let center = tensor
            .get(0, 0, 1)
            .expect("center entry should be readable");
        assert!(center.norm() > 0.0);
    }

    #[test]
    fn mmtrjas0_rejects_unknown_mu_lookup() {
        let tensor = mmtrjas0(Mmtrjas0Input {
            mu_values: &[-1, 0, 1],
            lind: &[0],
            eta_start: 0.0,
            eta_end: 0.0,
        })
        .expect("valid input should build tensor");

        let error = tensor.get(2, 0, 0).expect_err("unknown mu should fail");
        assert_eq!(error, TensorError::UnknownMu { mu: 2 });
    }
}
