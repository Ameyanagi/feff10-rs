use std::sync::OnceLock;

const IDIM: usize = 58;
const AL_DIM: usize = IDIM + 2;
static LOG_FACTORIALS: OnceLock<[f64; AL_DIM]> = OnceLock::new();

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum Cwig3jError {
    #[error("ient must be 1 or 2, got {0}")]
    InvalidIent(i32),
    #[error("argument {index}={value} is not divisible by ient={ient}")]
    ParityMismatch { index: usize, value: i32, ient: i32 },
    #[error("argument {index}={value} exceeds factorial lookup max={max}")]
    FactorialDomainOverflow { index: usize, value: i32, max: i32 },
    #[error("factorial lookup index {0} is out of bounds")]
    FactorialIndexOutOfRange(i32),
}

pub fn cwig3j(j1: i32, j2: i32, j3: i32, m1: i32, m2: i32, ient: i32) -> Result<f64, Cwig3jError> {
    let m3 = -m1 - m2;
    if !matches!(ient, 1 | 2) {
        return Err(Cwig3jError::InvalidIent(ient));
    }
    let ii = ient + ient;

    if m1.abs() + m2.abs() == 0 && (j1 + j2 + j3).rem_euclid(ii) != 0 {
        return Ok(0.0);
    }

    let mut m = [0i32; 12];
    m[0] = j1 + j2 - j3;
    m[1] = j2 + j3 - j1;
    m[2] = j3 + j1 - j2;
    m[3] = j1 + m1;
    m[4] = j1 - m1;
    m[5] = j2 + m2;
    m[6] = j2 - m2;
    m[7] = j3 + m3;
    m[8] = j3 - m3;
    m[9] = j1 + j2 + j3 + ient;
    m[10] = j2 - j3 - m1;
    m[11] = j1 - j3 + m2;

    for (index, value) in m.iter_mut().enumerate() {
        if index < 10 && *value < 0 {
            return Ok(0.0);
        }
        if value.rem_euclid(ient) != 0 {
            return Err(Cwig3jError::ParityMismatch {
                index: index + 1,
                value: *value,
                ient,
            });
        }
        *value /= ient;
        if *value > IDIM as i32 {
            return Err(Cwig3jError::FactorialDomainOverflow {
                index: index + 1,
                value: *value,
                max: IDIM as i32,
            });
        }
    }

    let max0 = m[10].max(m[11]).max(0) + 1;
    let min0 = m[0].min(m[4]).min(m[5]) + 1;

    let mut sign = 1.0f64;
    if (max0 - 1).rem_euclid(2) != 0 {
        sign = -sign;
    }

    let mut c = -lookup_log_factorial(m[9] + 1)?;
    for value in &m[..9] {
        c += lookup_log_factorial(*value + 1)?;
    }
    c *= 0.5;

    let mut value = 0.0f64;
    if max0 <= min0 {
        for i in max0..=min0 {
            let j = 2 - i;
            let b = lookup_log_factorial(i)?
                + lookup_log_factorial(j + m[0])?
                + lookup_log_factorial(j + m[4])?
                + lookup_log_factorial(j + m[5])?
                + lookup_log_factorial(i - m[10])?
                + lookup_log_factorial(i - m[11])?;
            value += sign * (c - b).exp();
            sign = -sign;
        }
    }

    if (j1 - j2 - m3).rem_euclid(ii) != 0 {
        value = -value;
    }

    Ok(value)
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

fn lookup_log_factorial(index: i32) -> Result<f64, Cwig3jError> {
    if !(1..=((IDIM + 1) as i32)).contains(&index) {
        return Err(Cwig3jError::FactorialIndexOutOfRange(index));
    }
    Ok(log_factorials()[index as usize])
}

#[cfg(test)]
mod tests {
    use super::{Cwig3jError, cwig3j};

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn integer_case_matches_known_closed_form_value() {
        let value = cwig3j(1, 1, 0, 0, 0, 1).expect("valid integer 3j");
        assert_close(value, -1.0 / 3.0f64.sqrt(), 1.0e-12);
    }

    #[test]
    fn half_integer_case_matches_known_closed_form_value() {
        let value = cwig3j(1, 1, 0, 1, -1, 2).expect("valid half-integer 3j");
        assert_close(value, 1.0 / 2.0f64.sqrt(), 1.0e-12);
    }

    #[test]
    fn parity_guard_returns_zero_for_invalid_total_angular_momentum() {
        let value = cwig3j(1, 1, 1, 0, 0, 1).expect("parity mismatch maps to zero");
        assert_close(value, 0.0, 1.0e-12);
    }

    #[test]
    fn invalid_ient_reports_error() {
        let err = cwig3j(1, 1, 0, 0, 0, 3).expect_err("ient outside supported domain");
        assert_eq!(err, Cwig3jError::InvalidIent(3));
    }
}
