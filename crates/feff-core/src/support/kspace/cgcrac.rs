#[derive(Debug, Clone, Copy)]
pub struct CgcracInput {
    pub j1: f64,
    pub j2: f64,
    pub j3: f64,
    pub m1: f64,
    pub m2: f64,
    pub m3: f64,
}

pub fn cgcrac(fact: &[f64], input: CgcracInput) -> f64 {
    let CgcracInput {
        j1,
        j2,
        j3,
        m1,
        m2,
        m3,
    } = input;

    if (m3 - (m1 + m2)).abs() > 1.0e-6
        || (j1 - j2).abs() > j3
        || (j1 + j2) < j3
        || m1.abs() > (j1 + 1.0e-6)
        || m2.abs() > (j2 + 1.0e-6)
        || m3.abs() > (j3 + 1.0e-6)
    {
        return 0.0;
    }

    let mut j = nint(2.0 * (j1 - j2)).abs();
    let upper = nint(2.0 * (j1 + j2));
    let target = nint(2.0 * j3);
    let mut found = false;
    while j <= upper {
        if j == target {
            found = true;
            break;
        }
        j += 2;
    }
    if !found {
        return 0.0;
    }

    let x = match (
        factorial_lookup(fact, j1 + j2 - j3),
        factorial_lookup(fact, j1 - j2 + j3),
        factorial_lookup(fact, -j1 + j2 + j3),
        factorial_lookup(fact, j1 + m1),
        factorial_lookup(fact, j1 - m1),
        factorial_lookup(fact, j2 + m2),
        factorial_lookup(fact, j2 - m2),
        factorial_lookup(fact, j3 + m3),
        factorial_lookup(fact, j3 - m3),
    ) {
        (
            Some(a0),
            Some(a1),
            Some(a2),
            Some(a3),
            Some(a4),
            Some(a5),
            Some(a6),
            Some(a7),
            Some(a8),
        ) => (2.0 * j3 + 1.0) * a0 * a1 * a2 * a3 * a4 * a5 * a6 * a7 * a8,
        _ => return 0.0,
    };

    let y = match factorial_lookup(fact, j1 + j2 + j3 + 1.0) {
        Some(value) => value,
        None => return 0.0,
    };
    if y <= 0.0 {
        return 0.0;
    }
    let vf = (x / y).sqrt();

    let n1 = nint(j1 + j2 - j3);
    let n2 = nint(j1 - m1);
    let n3 = nint(j2 + m2);
    let n4 = nint(j3 - j2 + m1);
    let n5 = nint(j3 - j1 - m2);

    let ntop = n1.min(n2).min(n3);
    let nbot = 0.max(-n4).max(-n5);
    if nbot > ntop {
        return 0.0;
    }

    let mut sign = if is_even(nbot + 1) { 1.0 } else { -1.0 };
    let mut sum = 0.0_f64;
    for n in nbot..=ntop {
        sign = -sign;
        let denominator = match (
            factorial_lookup(fact, n as f64),
            factorial_lookup(fact, (n1 - n) as f64),
            factorial_lookup(fact, (n2 - n) as f64),
            factorial_lookup(fact, (n3 - n) as f64),
            factorial_lookup(fact, (n4 + n) as f64),
            factorial_lookup(fact, (n5 + n) as f64),
        ) {
            (Some(a0), Some(a1), Some(a2), Some(a3), Some(a4), Some(a5)) => {
                a0 * a1 * a2 * a3 * a4 * a5
            }
            _ => return 0.0,
        };
        if denominator == 0.0 {
            return 0.0;
        }
        sum += sign / denominator;
    }

    vf * sum
}

fn nint(value: f64) -> i32 {
    value.round() as i32
}

fn factorial_lookup(fact: &[f64], value: f64) -> Option<f64> {
    let index = nint(value);
    if index < 0 {
        return None;
    }
    fact.get(index as usize).copied()
}

fn is_even(value: i32) -> bool {
    value & 1 == 0
}

#[cfg(test)]
mod tests {
    use super::{CgcracInput, cgcrac};
    use crate::support::kspace::factorial_table;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn zero_angular_momentum_case_is_unity() {
        let fact = factorial_table(100);
        let value = cgcrac(
            &fact,
            CgcracInput {
                j1: 0.0,
                j2: 0.0,
                j3: 0.0,
                m1: 0.0,
                m2: 0.0,
                m3: 0.0,
            },
        );
        assert_close(value, 1.0, 1.0e-12);
    }

    #[test]
    fn known_integer_case_matches_closed_form_value() {
        let fact = factorial_table(100);
        let value = cgcrac(
            &fact,
            CgcracInput {
                j1: 1.0,
                j2: 1.0,
                j3: 0.0,
                m1: 0.0,
                m2: 0.0,
                m3: 0.0,
            },
        );
        assert_close(value, -1.0 / 3.0_f64.sqrt(), 1.0e-12);
    }

    #[test]
    fn incompatible_m_projection_returns_zero() {
        let fact = factorial_table(100);
        let value = cgcrac(
            &fact,
            CgcracInput {
                j1: 1.0,
                j2: 1.0,
                j3: 1.0,
                m1: 1.0,
                m2: 1.0,
                m3: 1.0,
            },
        );
        assert_eq!(value, 0.0);
    }
}
