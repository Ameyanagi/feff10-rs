const IMAX0: i32 = 100;
const IMAX_STEP: i32 = 20;
const REL_TOLERANCE: f64 = 1.0e-10;
const MAX_EXPANSIONS: usize = 512;

pub fn strconfra(aa: f64, x: f64) -> f64 {
    if !aa.is_finite() || !x.is_finite() || x.abs() <= f64::EPSILON {
        return f64::NAN;
    }

    let mut imax = IMAX0 - IMAX_STEP;
    let mut previous = f64::NAN;

    for expansion in 0..MAX_EXPANSIONS {
        imax += IMAX_STEP;

        let mut value = imax as f64 / x;
        let mut i = imax;
        for _ in 2..=imax {
            value += 1.0;
            value = x + (i as f64 - aa) / value;
            value = (i as f64 - 1.0) / value;
            i -= 1;
        }

        value += 1.0;
        value = x + (1.0 - aa) / value;
        value = 1.0 / value;

        if expansion == 0 {
            previous = value;
            continue;
        }

        let denominator = value.abs().max(1.0e-30);
        if ((value - previous) / denominator).abs() <= REL_TOLERANCE {
            return value;
        }
        previous = value;
    }

    previous
}

#[cfg(test)]
mod tests {
    use super::strconfra;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn aa_equal_to_one_collapses_to_inverse_x() {
        let value = strconfra(1.0, 2.5);
        assert_close(value, 0.4, 1.0e-12);
    }

    #[test]
    fn converged_value_is_deterministic_for_same_input() {
        let first = strconfra(3.5, 4.25);
        let second = strconfra(3.5, 4.25);
        assert_close(first, second, 1.0e-14);
    }

    #[test]
    fn zero_divisor_input_returns_nan() {
        let value = strconfra(2.0, 0.0);
        assert!(value.is_nan());
    }
}
