use num_complex::Complex64;

pub const MAX_K_CHANNELS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LambdaIndex {
    pub m: i32,
    pub n: usize,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum TensorError {
    #[error("mu value {mu} is not part of the tensor basis")]
    UnknownMu { mu: i32 },
    #[error("k channel {channel} is out of range for channel_count={channel_count}")]
    InvalidChannel {
        channel: usize,
        channel_count: usize,
    },
    #[error("lambda index {index} is out of range for lam_count={lam_count}")]
    InvalidLambda { index: usize, lam_count: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct BmatiTensor {
    mu_values: Vec<i32>,
    channel_count: usize,
    data: Vec<Complex64>,
}

impl BmatiTensor {
    pub fn zeroed(mu_values: Vec<i32>, channel_count: usize) -> Self {
        let mu_count = mu_values.len();
        let len = mu_count
            .saturating_mul(channel_count)
            .saturating_mul(mu_count)
            .saturating_mul(channel_count);
        Self {
            mu_values,
            channel_count,
            data: vec![Complex64::new(0.0, 0.0); len],
        }
    }

    pub fn mu_values(&self) -> &[i32] {
        &self.mu_values
    }

    pub fn channel_count(&self) -> usize {
        self.channel_count
    }

    pub fn get(&self, mu1: i32, k1: usize, mu2: i32, k2: usize) -> Result<Complex64, TensorError> {
        let (mu1_index, mu2_index) = (self.mu_index(mu1)?, self.mu_index(mu2)?);
        let index = self.flat_index(mu1_index, k1, mu2_index, k2)?;
        Ok(self.data[index])
    }

    pub fn set(
        &mut self,
        mu1: i32,
        k1: usize,
        mu2: i32,
        k2: usize,
        value: Complex64,
    ) -> Result<(), TensorError> {
        let (mu1_index, mu2_index) = (self.mu_index(mu1)?, self.mu_index(mu2)?);
        let index = self.flat_index(mu1_index, k1, mu2_index, k2)?;
        self.data[index] = value;
        Ok(())
    }

    pub fn add(
        &mut self,
        mu1: i32,
        k1: usize,
        mu2: i32,
        k2: usize,
        value: Complex64,
    ) -> Result<(), TensorError> {
        let (mu1_index, mu2_index) = (self.mu_index(mu1)?, self.mu_index(mu2)?);
        let index = self.flat_index(mu1_index, k1, mu2_index, k2)?;
        self.data[index] += value;
        Ok(())
    }

    fn mu_index(&self, mu: i32) -> Result<usize, TensorError> {
        self.mu_values
            .iter()
            .position(|candidate| *candidate == mu)
            .ok_or(TensorError::UnknownMu { mu })
    }

    fn flat_index(
        &self,
        mu1_index: usize,
        k1: usize,
        mu2_index: usize,
        k2: usize,
    ) -> Result<usize, TensorError> {
        if k1 >= self.channel_count {
            return Err(TensorError::InvalidChannel {
                channel: k1,
                channel_count: self.channel_count,
            });
        }
        if k2 >= self.channel_count {
            return Err(TensorError::InvalidChannel {
                channel: k2,
                channel_count: self.channel_count,
            });
        }

        let mu_count = self.mu_values.len();
        let prefix = (mu1_index * self.channel_count + k1) * mu_count;
        Ok((prefix + mu2_index) * self.channel_count + k2)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FmatiMatrix {
    lam_count: usize,
    data: Vec<Complex64>,
}

impl FmatiMatrix {
    pub fn zeroed(lam_count: usize) -> Self {
        Self {
            lam_count,
            data: vec![Complex64::new(0.0, 0.0); lam_count.saturating_mul(lam_count)],
        }
    }

    pub fn lam_count(&self) -> usize {
        self.lam_count
    }

    pub fn get(&self, lam1: usize, lam2: usize) -> Result<Complex64, TensorError> {
        let index = self.flat_index(lam1, lam2)?;
        Ok(self.data[index])
    }

    pub fn set(&mut self, lam1: usize, lam2: usize, value: Complex64) -> Result<(), TensorError> {
        let index = self.flat_index(lam1, lam2)?;
        self.data[index] = value;
        Ok(())
    }

    fn flat_index(&self, lam1: usize, lam2: usize) -> Result<usize, TensorError> {
        if lam1 >= self.lam_count {
            return Err(TensorError::InvalidLambda {
                index: lam1,
                lam_count: self.lam_count,
            });
        }
        if lam2 >= self.lam_count {
            return Err(TensorError::InvalidLambda {
                index: lam2,
                lam_count: self.lam_count,
            });
        }
        Ok(lam1 * self.lam_count + lam2)
    }
}

pub fn contiguous_mu(max_abs_m: i32) -> Vec<i32> {
    if max_abs_m < 0 {
        return Vec::new();
    }
    (-max_abs_m..=max_abs_m).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedPathRecord {
    pub path_index: usize,
    pub nleg: usize,
    pub degeneracy: f64,
    pub reff: f64,
    pub cw_amplitude_ratio: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GenfmtArtifacts {
    pub feff_header_lines: Vec<String>,
    pub list_header_lines: Vec<String>,
    pub list_rows: Vec<String>,
    pub nstar_rows: Vec<String>,
}

impl GenfmtArtifacts {
    pub fn list_dat(&self) -> String {
        join_sections([&self.list_header_lines, &self.list_rows])
    }

    pub fn feff_header(&self) -> String {
        self.feff_header_lines.join("\n")
    }

    pub fn nstar_dat(&self) -> String {
        self.nstar_rows.join("\n")
    }
}

fn join_sections<'a>(sections: impl IntoIterator<Item = &'a Vec<String>>) -> String {
    let mut lines: Vec<&str> = Vec::new();
    for section in sections {
        lines.extend(section.iter().map(String::as_str));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{BmatiTensor, FmatiMatrix, TensorError, contiguous_mu};
    use num_complex::Complex64;

    #[test]
    fn contiguous_mu_builds_symmetric_basis() {
        let values = contiguous_mu(2);
        assert_eq!(values, vec![-2, -1, 0, 1, 2]);
        assert_eq!(contiguous_mu(-1), Vec::<i32>::new());
    }

    #[test]
    fn bmati_tensor_round_trip_supports_addition() {
        let mut tensor = BmatiTensor::zeroed(vec![-1, 0, 1], 2);
        tensor
            .set(-1, 0, 1, 1, Complex64::new(1.0, -2.0))
            .expect("valid set should succeed");
        tensor
            .add(-1, 0, 1, 1, Complex64::new(0.5, 0.5))
            .expect("valid add should succeed");

        let value = tensor
            .get(-1, 0, 1, 1)
            .expect("stored value should be readable");
        assert_eq!(value, Complex64::new(1.5, -1.5));
    }

    #[test]
    fn bmati_tensor_rejects_unknown_mu() {
        let tensor = BmatiTensor::zeroed(vec![-1, 0, 1], 1);
        let error = tensor.get(2, 0, 0, 0).expect_err("unknown mu should fail");
        assert_eq!(error, TensorError::UnknownMu { mu: 2 });
    }

    #[test]
    fn fmati_matrix_round_trip_works() {
        let mut matrix = FmatiMatrix::zeroed(3);
        matrix
            .set(1, 2, Complex64::new(-3.0, 4.0))
            .expect("valid set should succeed");

        let value = matrix.get(1, 2).expect("stored value should be readable");
        assert_eq!(value, Complex64::new(-3.0, 4.0));
    }

    #[test]
    fn fmati_matrix_rejects_out_of_range_lambda() {
        let matrix = FmatiMatrix::zeroed(2);
        let error = matrix
            .get(2, 1)
            .expect_err("out-of-range lambda should fail");
        assert_eq!(
            error,
            TensorError::InvalidLambda {
                index: 2,
                lam_count: 2
            }
        );
    }
}
