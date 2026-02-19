use num_complex::Complex64;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SclmzError {
    #[error("lmaxp1 and mmaxp1 must both be positive")]
    InvalidGrid,
    #[error("rho must be non-zero")]
    ZeroRho,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SclmzOutput {
    pub z: Complex64,
    pub clmi: Vec<Vec<Complex64>>,
}

pub fn sclmz(rho: Complex64, lmaxp1: usize, mmaxp1: usize) -> Result<SclmzOutput, SclmzError> {
    if lmaxp1 == 0 || mmaxp1 == 0 {
        return Err(SclmzError::InvalidGrid);
    }
    if rho.norm() <= f64::EPSILON {
        return Err(SclmzError::ZeroRho);
    }

    let mut clmi = vec![vec![Complex64::new(0.0, 0.0); mmaxp1]; lmaxp1];
    let z = -Complex64::new(0.0, 1.0) / rho;

    clmi[0][0] = Complex64::new(1.0, 0.0);
    if lmaxp1 > 1 {
        clmi[1][0] = clmi[0][0] - z;
    }

    let lmax = lmaxp1 - 1;
    for l in 2..=lmax {
        clmi[l][0] = clmi[l - 2][0] - z * (2 * l - 1) as f64 * clmi[l - 1][0];
    }

    let mmxp1 = mmaxp1.min(lmaxp1);
    let mut cmm = Complex64::new(1.0, 0.0);

    for im in 1..mmxp1 {
        let m = im;
        cmm = -cmm * (2 * m - 1) as f64 * z;
        clmi[m][m] = cmm;

        if m < lmax {
            clmi[m + 1][m] =
                cmm * (2 * m + 1) as f64 * (Complex64::new(1.0, 0.0) - z * (m + 1) as f64);
        }

        for l in (m + 1)..lmax {
            clmi[l + 1][m] =
                clmi[l - 1][m] - z * (2 * l + 1) as f64 * (clmi[l][m] + clmi[l][m - 1]);
        }
    }

    Ok(SclmzOutput { z, clmi })
}

#[cfg(test)]
mod tests {
    use super::{SclmzError, sclmz};
    use num_complex::Complex64;

    #[test]
    fn sclmz_seeds_and_recurs_coefficients() {
        let output =
            sclmz(Complex64::new(2.0, 0.5), 5, 4).expect("valid rho should build coefficients");

        assert_eq!(output.clmi[0][0], Complex64::new(1.0, 0.0));
        assert!(output.clmi[2][0].norm() > 0.0);
        assert!(output.clmi[3][1].norm() > 0.0);
    }

    #[test]
    fn sclmz_respects_triangular_domain() {
        let output =
            sclmz(Complex64::new(1.0, 0.0), 4, 6).expect("valid rho should build coefficients");

        assert_eq!(output.clmi[0][1], Complex64::new(0.0, 0.0));
        assert_eq!(output.clmi[1][2], Complex64::new(0.0, 0.0));
    }

    #[test]
    fn sclmz_rejects_zero_rho() {
        let error = sclmz(Complex64::new(0.0, 0.0), 3, 3).expect_err("zero rho should fail");
        assert_eq!(error, SclmzError::ZeroRho);
    }
}
