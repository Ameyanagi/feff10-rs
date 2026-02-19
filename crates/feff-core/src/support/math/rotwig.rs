use std::sync::OnceLock;

const IDIM: usize = 58;
const AL_DIM: usize = IDIM + 2;
static LOG_FACTORIALS: OnceLock<[f64; AL_DIM]> = OnceLock::new();

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum RotwigError {
    #[error("ient must be 1 or 2, got {0}")]
    InvalidIent(i32),
    #[error("expression `{expression}`={value} is not divisible by ient={ient}")]
    ParityMismatch {
        expression: &'static str,
        value: i32,
        ient: i32,
    },
    #[error("factorial lookup argument m({index})={value} exceeds max={max}")]
    FactorialDomainOverflow { index: usize, value: i32, max: i32 },
    #[error("factorial lookup index {0} is out of bounds")]
    FactorialIndexOutOfRange(i32),
}

pub fn rotwig(beta: f64, jj: i32, m1: i32, m2: i32, ient: i32) -> Result<f64, RotwigError> {
    if !matches!(ient, 1 | 2) {
        return Err(RotwigError::InvalidIent(ient));
    }

    let (m1p, m2p, betap, isign) = if m1 >= 0 && m1.abs() >= m2.abs() {
        (m1, m2, beta, 1.0)
    } else if m2 >= 0 && m2.abs() >= m1.abs() {
        (m2, m1, -beta, 1.0)
    } else if m1 <= 0 && m1.abs() >= m2.abs() {
        let exponent = checked_div("m1-m2", m1 - m2, ient)?;
        (-m1, -m2, beta, parity_sign(exponent))
    } else {
        let exponent = checked_div("m2-m1", m2 - m1, ient)?;
        (-m2, -m1, -beta, parity_sign(exponent))
    };

    let zeta = (betap / 2.0).cos();
    let eta = (betap / 2.0).sin();

    let mut temp = 0.0f64;
    let start = m1p - m2p;
    let end = jj - m2p;
    if start <= end {
        let mut it = start;
        while it <= end {
            let m = [
                1 + checked_div("jj+m1p", jj + m1p, ient)?,
                1 + checked_div("jj-m1p", jj - m1p, ient)?,
                1 + checked_div("jj+m2p", jj + m2p, ient)?,
                1 + checked_div("jj-m2p", jj - m2p, ient)?,
                1 + checked_div("jj+m1p-it", jj + m1p - it, ient)?,
                1 + checked_div("jj-m2p-it", jj - m2p - it, ient)?,
                1 + checked_div("it", it, ient)?,
                1 + checked_div("m2p-m1p+it", m2p - m1p + it, ient)?,
            ];

            for (index, value) in m.iter().enumerate() {
                if *value > (IDIM + 1) as i32 {
                    return Err(RotwigError::FactorialDomainOverflow {
                        index: index + 1,
                        value: *value,
                        max: (IDIM + 1) as i32,
                    });
                }
            }

            let m9 = checked_div("2*jj+m1p-m2p-2*it", 2 * jj + m1p - m2p - 2 * it, ient)?;
            let m10 = checked_div("2*it-m1p+m2p", 2 * it - m1p + m2p, ient)?;
            let phase = checked_div("it", it, ient)?;

            let mut factor = 0.0f64;
            for index in 0..4 {
                factor +=
                    lookup_log_factorial(m[index])? / 2.0 - lookup_log_factorial(m[index + 4])?;
            }

            let mut contribution = parity_sign(phase) * factor.exp();
            if m9 != 0 {
                contribution *= zeta.powi(m9);
            }
            if m10 != 0 {
                contribution *= eta.powi(m10);
            }
            temp += contribution;

            it += ient;
        }
    }

    Ok(isign * temp)
}

fn checked_div(expression: &'static str, value: i32, ient: i32) -> Result<i32, RotwigError> {
    if value.rem_euclid(ient) != 0 {
        return Err(RotwigError::ParityMismatch {
            expression,
            value,
            ient,
        });
    }
    Ok(value / ient)
}

fn parity_sign(exponent: i32) -> f64 {
    if exponent.rem_euclid(2) == 0 {
        1.0
    } else {
        -1.0
    }
}

fn log_factorials() -> &'static [f64; AL_DIM] {
    LOG_FACTORIALS.get_or_init(|| {
        let mut values = [0.0f64; AL_DIM];
        values[1] = 0.0;
        for i in 1..=IDIM {
            values[i + 1] = values[i] + (i as f64).ln();
        }
        values
    })
}

fn lookup_log_factorial(index: i32) -> Result<f64, RotwigError> {
    if !(1..=((IDIM + 1) as i32)).contains(&index) {
        return Err(RotwigError::FactorialIndexOutOfRange(index));
    }
    Ok(log_factorials()[index as usize])
}

#[cfg(test)]
mod tests {
    use super::{RotwigError, rotwig};
    use std::f64::consts::PI;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn beta_zero_diagonal_term_is_unity() {
        let value = rotwig(0.0, 1, 1, 1, 1).expect("valid integer case");
        assert_close(value, 1.0, 1.0e-12);
    }

    #[test]
    fn integer_case_matches_known_closed_form() {
        let beta = PI / 3.0;
        let value = rotwig(beta, 1, 1, 1, 1).expect("valid integer case");
        assert_close(value, (beta / 2.0).cos().powi(2), 1.0e-12);
    }

    #[test]
    fn half_integer_case_matches_known_closed_form() {
        let beta = PI / 3.0;
        let value = rotwig(beta, 1, 1, 1, 2).expect("valid half-integer case");
        assert_close(value, (beta / 2.0).cos(), 1.0e-12);
    }

    #[test]
    fn beta_zero_off_diagonal_term_is_zero() {
        let value = rotwig(0.0, 2, 1, -1, 1).expect("valid integer case");
        assert_close(value, 0.0, 1.0e-12);
    }

    #[test]
    fn invalid_ient_reports_error() {
        let err = rotwig(0.0, 1, 1, 1, 3).expect_err("invalid ient");
        assert_eq!(err, RotwigError::InvalidIent(3));
    }

    #[test]
    fn parity_mismatch_reports_error() {
        let err = rotwig(0.1, 1, 1, 0, 2).expect_err("invalid parity");
        assert!(matches!(
            err,
            RotwigError::ParityMismatch {
                expression: "jj+m2p",
                value: 1,
                ient: 2
            }
        ));
    }
}
