#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum SnlmError {
    #[error("lmaxp1 and mmaxp1 must both be positive")]
    InvalidGrid,
    #[error(
        "requested lmax={lmax} requires factorial index {required}, above supported limit {limit}"
    )]
    FactorialOutOfRange {
        lmax: usize,
        required: usize,
        limit: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SnlmOutput {
    pub afac: f64,
    pub xnlm: Vec<Vec<f64>>,
}

pub fn snlm(lmaxp1: usize, mmaxp1: usize) -> Result<SnlmOutput, SnlmError> {
    if lmaxp1 == 0 || mmaxp1 == 0 {
        return Err(SnlmError::InvalidGrid);
    }

    let lmax = lmaxp1 - 1;
    let required_factorial_index = lmax.saturating_mul(2);
    const FACTORIAL_LIMIT: usize = 210;
    if required_factorial_index > FACTORIAL_LIMIT {
        return Err(SnlmError::FactorialOutOfRange {
            lmax,
            required: required_factorial_index,
            limit: FACTORIAL_LIMIT,
        });
    }

    let (afac, flg) = factst(required_factorial_index);
    let mut xnlm = vec![vec![0.0; mmaxp1]; lmaxp1];

    for (il, row) in xnlm.iter_mut().enumerate() {
        let mmxp1 = mmaxp1.min(il + 1);
        for (im, entry) in row.iter_mut().enumerate().take(mmxp1) {
            let l = il;
            let m = im;
            let cnlm = (2 * l + 1) as f64 * flg[l - m] / flg[l + m];
            *entry = cnlm.sqrt() * afac.powi(m as i32);
        }
    }

    Ok(SnlmOutput { afac, xnlm })
}

fn factst(limit: usize) -> (f64, Vec<f64>) {
    let afac = 1.0 / 64.0;
    let mut flg = vec![1.0; limit + 1];
    if limit >= 1 {
        flg[1] = afac;
    }

    for i in 2..=limit {
        flg[i] = flg[i - 1] * i as f64 * afac;
    }

    (afac, flg)
}

#[cfg(test)]
mod tests {
    use super::{SnlmError, snlm};

    #[test]
    fn snlm_reproduces_known_low_order_norms() {
        let output = snlm(4, 4).expect("valid dimensions should build normalization table");

        assert!((output.xnlm[0][0] - 1.0).abs() < 1.0e-12);
        assert!((output.xnlm[1][0] - 3.0_f64.sqrt()).abs() < 1.0e-12);

        let expected_l1_m1 = (1.5_f64).sqrt();
        assert!((output.xnlm[1][1] - expected_l1_m1).abs() < 1.0e-12);
    }

    #[test]
    fn snlm_zero_fills_outside_triangular_lm_domain() {
        let output = snlm(3, 5).expect("valid dimensions should build table");

        assert_eq!(output.xnlm[0][1], 0.0);
        assert_eq!(output.xnlm[1][2], 0.0);
        assert_eq!(output.xnlm[2][4], 0.0);
    }

    #[test]
    fn snlm_rejects_non_positive_grid() {
        let error = snlm(0, 3).expect_err("invalid dimensions should fail");
        assert_eq!(error, SnlmError::InvalidGrid);
    }
}
