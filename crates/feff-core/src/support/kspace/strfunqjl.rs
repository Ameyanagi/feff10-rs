use std::f64::consts::PI;

pub fn strfunqjl(fact: &[f64], j: usize, l: usize) -> f64 {
    let ratio = if j == 0 {
        0.5
    } else if l < j {
        return 0.0;
    } else {
        let numerator = match fact.get(l - j) {
            Some(value) => *value,
            None => return 0.0,
        };
        let denominator = match fact.get(l + j) {
            Some(value) => *value,
            None => return 0.0,
        };
        if denominator == 0.0 {
            return 0.0;
        }
        numerator / denominator
    };

    let value = ratio * (2.0 * l as f64 + 1.0) / (2.0 * PI);
    if value <= 0.0 { 0.0 } else { value.sqrt() }
}

#[cfg(test)]
mod tests {
    use super::strfunqjl;
    use crate::support::kspace::factorial_table;
    use std::f64::consts::PI;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn j_zero_matches_constant_prefactor_formula() {
        let fact = factorial_table(100);
        let value = strfunqjl(&fact, 0, 2);
        let expected = (5.0 / (4.0 * PI)).sqrt();
        assert_close(value, expected, 1.0e-12);
    }

    #[test]
    fn non_zero_j_uses_factorial_ratio() {
        let fact = factorial_table(100);
        let value = strfunqjl(&fact, 1, 2);
        let expected = (5.0 / (12.0 * PI)).sqrt();
        assert_close(value, expected, 1.0e-12);
    }

    #[test]
    fn j_l_out_of_domain_returns_zero() {
        let fact = factorial_table(100);
        let value = strfunqjl(&fact, 3, 2);
        assert_eq!(value, 0.0);
    }
}
