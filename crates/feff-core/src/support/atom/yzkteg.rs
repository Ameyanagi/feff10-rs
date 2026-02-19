#[derive(Debug, Clone)]
pub struct YzktegInput<'a> {
    pub f: &'a [f64],
    pub af: &'a [f64],
    pub dr: &'a [f64],
    pub ap: f64,
    pub h: f64,
    pub k: i32,
    pub nd: usize,
    pub np: usize,
    pub idim: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct YzktegOutput {
    pub yk: Vec<f64>,
    pub yk_dev: Vec<f64>,
    pub zk: Vec<f64>,
    pub zk_dev: Vec<f64>,
    pub ap: f64,
    pub np: usize,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum YzktegError {
    #[error("idim must be >= 5, got {0}")]
    InvalidDimension(usize),
    #[error("nd must be >= 1")]
    InvalidSeriesOrder,
    #[error("integration uses np={np}, but at least 3 points are required")]
    InvalidPointCount { np: usize },
    #[error("input length mismatch for {name}: need at least {need}, got {got}")]
    LengthMismatch {
        name: &'static str,
        need: usize,
        got: usize,
    },
    #[error("dr(1) must be > 0, got {0}")]
    NonPositiveFirstRadius(f64),
    #[error("development denominator vanished at term {term_index}")]
    SingularDevelopmentTerm { term_index: usize },
}

pub fn yzkteg(input: &YzktegInput<'_>) -> Result<YzktegOutput, YzktegError> {
    if input.idim < 5 {
        return Err(YzktegError::InvalidDimension(input.idim));
    }
    if input.nd == 0 {
        return Err(YzktegError::InvalidSeriesOrder);
    }
    ensure_len("f", input.f.len(), input.idim)?;
    ensure_len("af", input.af.len(), input.nd)?;
    ensure_len("dr", input.dr.len(), input.idim)?;
    if input.dr[0] <= 0.0 {
        return Err(YzktegError::NonPositiveFirstRadius(input.dr[0]));
    }

    let np = input.np.min(input.idim.saturating_sub(2));
    if np < 3 {
        return Err(YzktegError::InvalidPointCount { np });
    }

    let mut yk = vec![0.0_f64; input.idim];
    let mut yk_dev = vec![0.0_f64; input.nd];
    let mut zk = vec![0.0_f64; input.idim];
    let mut zk_dev = vec![0.0_f64; input.nd];

    let mut b = input.ap;
    let mut ap = 0.0_f64;

    let mut idx = 0usize;
    while idx < input.nd {
        b += 1.0;

        let denom_zk = b + input.k as f64;
        if input.af[idx] != 0.0 {
            let denom_yk = b - input.k as f64 - 1.0;
            if denom_zk == 0.0 || denom_yk == 0.0 {
                return Err(YzktegError::SingularDevelopmentTerm {
                    term_index: idx + 1,
                });
            }

            zk_dev[idx] = input.af[idx] / denom_zk;
            let c0 = input.dr[0].powf(b);
            zk[0] += zk_dev[idx] * c0;
            zk[1] += zk_dev[idx] * input.dr[1].powf(b);

            yk_dev[idx] = (input.k + input.k + 1) as f64 * zk_dev[idx] / denom_yk;
            ap += yk_dev[idx] * c0;
        }
        idx += 1;
    }

    let mut i = 0usize;
    while i < np {
        yk[i] = input.f[i] * input.dr[i];
        i += 1;
    }
    yk[np] = 0.0;
    yk[np + 1] = 0.0;

    let eh = input.h.exp();
    let e = eh.powf(-(input.k as f64));
    let mut b4 = input.h / 24.0;
    let c4 = 13.0 * b4;
    let mut ee4 = e * e * b4;
    b4 /= e;

    let mut index = 2usize;
    while index <= np {
        zk[index] = zk[index - 1] * e
            + (c4 * (yk[index] + yk[index - 1] * e) - (yk[index - 2] * ee4 + yk[index + 1] * b4));
        index += 1;
    }

    yk[np - 1] = zk[np - 1];
    let mut tail = np;
    while tail < input.idim {
        yk[tail] = yk[tail - 1] * e;
        tail += 1;
    }

    let ik = (input.k + input.k + 1) as f64;
    b4 = ik * b4 * eh;
    ee4 = ik * ee4 / (eh * eh);
    let eb = e / eh;
    let cback = ik * c4;

    let mut back = np - 1;
    while back >= 2 {
        yk[back - 1] = yk[back] * eb
            + (cback * (zk[back - 1] + zk[back] * eb) - (zk[back + 1] * ee4 + zk[back - 2] * b4));
        back -= 1;
    }

    let ee_start = eb * eb;
    let cstart = 8.0 * cback / 13.0;
    yk[0] = yk[2] * ee_start + cstart * (zk[2] * ee_start + 4.0 * eb * zk[1] + zk[0]);

    ap = (ap + yk[0]) / input.dr[0].powf((input.k + 1) as f64);

    Ok(YzktegOutput {
        yk,
        yk_dev,
        zk,
        zk_dev,
        ap,
        np,
    })
}

fn ensure_len(name: &'static str, got: usize, need: usize) -> Result<(), YzktegError> {
    if got < need {
        return Err(YzktegError::LengthMismatch { name, need, got });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{YzktegError, YzktegInput, yzkteg};

    #[test]
    fn yzkteg_returns_zero_for_zero_source_and_coefficients() {
        let input = YzktegInput {
            f: &[0.0; 8],
            af: &[0.0; 3],
            dr: &[1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7],
            ap: 0.0,
            h: 0.05,
            k: 0,
            nd: 3,
            np: 6,
            idim: 8,
        };

        let output = yzkteg(&input).expect("yzkteg should succeed");
        assert!(output.yk.iter().all(|value| value.abs() <= 1.0e-12));
        assert!(output.zk.iter().all(|value| value.abs() <= 1.0e-12));
        assert!(output.yk_dev.iter().all(|value| value.abs() <= 1.0e-12));
        assert!(output.zk_dev.iter().all(|value| value.abs() <= 1.0e-12));
        assert!(output.ap.abs() <= 1.0e-12);
    }

    #[test]
    fn yzkteg_builds_non_trivial_integrals_for_nonzero_source() {
        let input = YzktegInput {
            f: &[1.0, 0.5, 0.25, 0.125, 0.0, 0.0, 0.0, 0.0],
            af: &[0.2, 0.1, 0.0],
            dr: &[0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5],
            ap: 2.0,
            h: 0.04,
            k: 1,
            nd: 3,
            np: 6,
            idim: 8,
        };

        let output = yzkteg(&input).expect("yzkteg should succeed");

        assert!(output.yk[0].is_finite());
        assert!(output.zk[0].is_finite());
        assert!(output.ap.is_finite());
        assert!(output.yk.iter().any(|value| value.abs() > 1.0e-9));
        assert!(output.zk.iter().any(|value| value.abs() > 1.0e-9));
    }

    #[test]
    fn yzkteg_rejects_small_np_after_dimension_clamp() {
        let input = YzktegInput {
            f: &[0.0; 5],
            af: &[0.0; 1],
            dr: &[1.0; 5],
            ap: 0.0,
            h: 0.1,
            k: 0,
            nd: 1,
            np: 2,
            idim: 5,
        };

        let error = yzkteg(&input).expect_err("np<3 should fail");
        assert_eq!(error, YzktegError::InvalidPointCount { np: 2 });
    }
}
