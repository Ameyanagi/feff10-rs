use num_complex::Complex64;

const TOL: f64 = 1.0e-10;

pub fn xlogx(x: f64) -> Complex64 {
    if x.abs() > TOL {
        let xc = Complex64::new(x, 0.0);
        xc * xc.ln()
    } else if x > 0.0 {
        Complex64::new(x * TOL.ln(), 0.0)
    } else {
        Complex64::new(0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::xlogx;
    use num_complex::Complex64;
    use std::f64::consts::PI;

    fn assert_close(actual: Complex64, expected: Complex64, tolerance: f64) {
        assert!(
            (actual - expected).norm() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn positive_argument_matches_x_times_log_x() {
        let x = 2.0;
        let value = xlogx(x);
        assert_close(value, Complex64::new(x * x.ln(), 0.0), 1.0e-12);
    }

    #[test]
    fn small_positive_argument_uses_linearized_branch() {
        let x = 1.0e-12;
        let value = xlogx(x);
        assert_close(value, Complex64::new(x * (1.0e-10f64).ln(), 0.0), 1.0e-20);
    }

    #[test]
    fn zero_argument_returns_zero() {
        assert_close(xlogx(0.0), Complex64::new(0.0, 0.0), 1.0e-24);
    }

    #[test]
    fn negative_argument_uses_complex_log_branch() {
        let value = xlogx(-2.0);
        assert_close(
            value,
            Complex64::new(-2.0 * (2.0f64).ln(), -2.0 * PI),
            1.0e-12,
        );
    }
}
