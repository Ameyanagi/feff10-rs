const MAX_ITERATIONS: usize = 10_000;

#[derive(Debug, Clone, thiserror::Error)]
pub enum EllfunError {
    #[error("invalid arguments in rf: x={x}, y={y}, z={z}")]
    InvalidRfArguments { x: f64, y: f64, z: f64 },
    #[error("invalid arguments in rj: x={x}, y={y}, z={z}, p={p}")]
    InvalidRjArguments { x: f64, y: f64, z: f64, p: f64 },
    #[error("invalid arguments in rc: x={x}, y={y}")]
    InvalidRcArguments { x: f64, y: f64 },
    #[error("{routine} failed to converge within {iterations} iterations")]
    NonConvergent {
        routine: &'static str,
        iterations: usize,
    },
}

pub fn ellpi(x: f64) -> Result<f64, EllfunError> {
    let en = -x;
    Ok(rf(0.0, 0.5, 1.0)? - en / 3.0 * rj(0.0, 0.5, 1.0, 1.0 + en)?)
}

pub fn rf(x: f64, y: f64, z: f64) -> Result<f64, EllfunError> {
    const ERRTOL: f64 = 0.0025;
    const TINY: f64 = 1.5e-38;
    const BIG: f64 = 3.0e37;
    const THIRD: f64 = 1.0 / 3.0;
    const C1: f64 = 1.0 / 24.0;
    const C2: f64 = 0.1;
    const C3: f64 = 3.0 / 44.0;
    const C4: f64 = 1.0 / 14.0;

    let min_xyz = x.min(y).min(z);
    let min_pair = (x + y).min(x + z).min(y + z);
    let max_xyz = x.max(y).max(z);
    if min_xyz < 0.0 || min_pair < TINY || max_xyz > BIG {
        return Err(EllfunError::InvalidRfArguments { x, y, z });
    }

    let mut xt = x;
    let mut yt = y;
    let mut zt = z;
    for _ in 0..MAX_ITERATIONS {
        let sqrtx = xt.sqrt();
        let sqrty = yt.sqrt();
        let sqrtz = zt.sqrt();
        let alamb = sqrtx * (sqrty + sqrtz) + sqrty * sqrtz;
        xt = 0.25 * (xt + alamb);
        yt = 0.25 * (yt + alamb);
        zt = 0.25 * (zt + alamb);
        let ave = THIRD * (xt + yt + zt);
        let delx = (ave - xt) / ave;
        let dely = (ave - yt) / ave;
        let delz = (ave - zt) / ave;
        if delx.abs().max(dely.abs()).max(delz.abs()) <= ERRTOL {
            let e2 = delx * dely - delz.powi(2);
            let e3 = delx * dely * delz;
            return Ok((1.0 + (C1 * e2 - C2 - C3 * e3) * e2 + C4 * e3) / ave.sqrt());
        }
    }

    Err(EllfunError::NonConvergent {
        routine: "rf",
        iterations: MAX_ITERATIONS,
    })
}

pub fn rj(x: f64, y: f64, z: f64, p: f64) -> Result<f64, EllfunError> {
    const ERRTOL: f64 = 0.0015;
    const TINY: f64 = 2.5e-13;
    const BIG: f64 = 9.0e11;
    const C1: f64 = 3.0 / 14.0;
    const C2: f64 = 1.0 / 3.0;
    const C3: f64 = 3.0 / 22.0;
    const C4: f64 = 3.0 / 26.0;
    const C5: f64 = 0.75 * C3;
    const C6: f64 = 1.5 * C4;
    const C7: f64 = 0.5 * C2;
    const C8: f64 = C3 + C3;

    let min_xyz = x.min(y).min(z);
    let min_pair = (x + y).min(x + z).min(y + z).min(p.abs());
    let max_xyz = x.max(y).max(z).max(p.abs());
    if min_xyz < 0.0 || min_pair < TINY || max_xyz > BIG {
        return Err(EllfunError::InvalidRjArguments { x, y, z, p });
    }

    let mut sum = 0.0f64;
    let mut fac = 1.0f64;
    let mut a = 0.0f64;
    let mut b = 0.0f64;
    let mut rcx = 0.0f64;

    let mut xt;
    let mut yt;
    let mut zt;
    let mut pt;

    if p > 0.0 {
        xt = x;
        yt = y;
        zt = z;
        pt = p;
    } else {
        let mut ordered = [x, y, z];
        ordered.sort_by(|lhs, rhs| lhs.total_cmp(rhs));
        xt = ordered[0];
        yt = ordered[1];
        zt = ordered[2];
        a = 1.0 / (yt - p);
        b = a * (zt - yt) * (yt - xt);
        pt = yt + b;
        let rho = xt * zt / yt;
        let tau = p * pt / yt;
        rcx = rc(rho, tau)?;
    }

    for _ in 0..MAX_ITERATIONS {
        let sqrtx = xt.sqrt();
        let sqrty = yt.sqrt();
        let sqrtz = zt.sqrt();
        let alamb = sqrtx * (sqrty + sqrtz) + sqrty * sqrtz;
        let alpha = (pt * (sqrtx + sqrty + sqrtz) + sqrtx * sqrty * sqrtz).powi(2);
        let beta = pt * (pt + alamb).powi(2);
        sum += fac * rc(alpha, beta)?;
        fac *= 0.25;
        xt = 0.25 * (xt + alamb);
        yt = 0.25 * (yt + alamb);
        zt = 0.25 * (zt + alamb);
        pt = 0.25 * (pt + alamb);
        let ave = 0.2 * (xt + yt + zt + pt + pt);
        let delx = (ave - xt) / ave;
        let dely = (ave - yt) / ave;
        let delz = (ave - zt) / ave;
        let delp = (ave - pt) / ave;
        if delx.abs().max(dely.abs()).max(delz.abs()).max(delp.abs()) <= ERRTOL {
            let ea = delx * (dely + delz) + dely * delz;
            let eb = delx * dely * delz;
            let ec = delp.powi(2);
            let ed = ea - 3.0 * ec;
            let ee = eb + 2.0 * delp * (ea - ec);
            let mut value = 3.0 * sum
                + fac
                    * (1.0
                        + ed * (-C1 + C5 * ed - C6 * ee)
                        + eb * (C7 + delp * (-C8 + delp * C4))
                        + delp * ea * (C2 - delp * C3)
                        - C2 * delp * ec)
                    / (ave * ave.sqrt());
            if p <= 0.0 {
                value = a * (b * value + 3.0 * (rcx - rf(xt, yt, zt)?));
            }
            return Ok(value);
        }
    }

    Err(EllfunError::NonConvergent {
        routine: "rj",
        iterations: MAX_ITERATIONS,
    })
}

pub fn rc(x: f64, y: f64) -> Result<f64, EllfunError> {
    const ERRTOL: f64 = 0.0012;
    const TINY: f64 = 1.69e-38;
    const SQRTNY: f64 = 1.3e-19;
    const BIG: f64 = 3.0e37;
    const TNBG: f64 = TINY * BIG;
    const COMP1: f64 = 2.236 / SQRTNY;
    const COMP2: f64 = TNBG * TNBG / 25.0;
    const THIRD: f64 = 1.0 / 3.0;
    const C1: f64 = 0.3;
    const C2: f64 = 1.0 / 3.0;
    const C3: f64 = 0.375;
    const C4: f64 = 9.0 / 22.0;

    if x < 0.0
        || y == 0.0
        || (x + y.abs()) < TINY
        || (x + y.abs()) > BIG
        || (y < -COMP1 && x > 0.0 && x < COMP2)
    {
        return Err(EllfunError::InvalidRcArguments { x, y });
    }

    let mut xt;
    let mut yt;
    let w;
    if y > 0.0 {
        xt = x;
        yt = y;
        w = 1.0;
    } else {
        xt = x - y;
        yt = -y;
        w = x.sqrt() / xt.sqrt();
    }

    for _ in 0..MAX_ITERATIONS {
        let alamb = 2.0 * xt.sqrt() * yt.sqrt() + yt;
        xt = 0.25 * (xt + alamb);
        yt = 0.25 * (yt + alamb);
        let ave = THIRD * (xt + yt + yt);
        let s = (yt - ave) / ave;
        if s.abs() <= ERRTOL {
            return Ok(w * (1.0 + s.powi(2) * (C1 + s * (C2 + s * (C3 + s * C4)))) / ave.sqrt());
        }
    }

    Err(EllfunError::NonConvergent {
        routine: "rc",
        iterations: MAX_ITERATIONS,
    })
}

#[cfg(test)]
mod tests {
    use super::{EllfunError, ellpi, rc, rf, rj};
    use std::f64::consts::PI;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn rc_matches_closed_form_value() {
        let value = rc(0.0, 0.25).expect("valid rc arguments");
        assert_close(value, PI, 1.0e-9);
    }

    #[test]
    fn rf_matches_reference_value() {
        let value = rf(0.0, 1.0, 2.0).expect("valid rf arguments");
        assert_close(value, 1.311_028_777_146_059_9, 1.0e-9);
    }

    #[test]
    fn rj_matches_reference_value() {
        let value = rj(0.0, 0.5, 1.0, 1.2).expect("valid rj arguments");
        assert_close(value, 2.621_142_532_828_929, 1.0e-9);
    }

    #[test]
    fn ellpi_matches_reference_value() {
        let value = ellpi(0.2).expect("valid ellpi argument");
        assert_close(value, 2.092_956_582_731_875_5, 1.0e-9);
    }

    #[test]
    fn ellpi_zero_matches_rf_constant_term() {
        let ellpi_zero = ellpi(0.0).expect("valid ellpi argument");
        let rf_value = rf(0.0, 0.5, 1.0).expect("valid rf arguments");
        assert_close(ellpi_zero, rf_value, 1.0e-9);
    }

    #[test]
    fn invalid_rc_arguments_return_error() {
        let err = rc(-1.0, 1.0).expect_err("invalid rc arguments");
        assert!(matches!(err, EllfunError::InvalidRcArguments { .. }));
    }
}
