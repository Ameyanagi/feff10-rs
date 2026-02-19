pub fn ffq(q: f64, ef: f64, xk: f64, wp: f64, alph: f64) -> f64 {
    let wq = (wp.powi(2) + alph * q.powi(2) + q.powi(4)).sqrt();
    let value = (wp + wq) / q.powi(2) + alph / (2.0 * wp);
    ((ef * wp) / (4.0 * xk)) * value.ln()
}

#[cfg(test)]
mod tests {
    use super::ffq;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn sample_inputs_match_reference_value() {
        let value = ffq(1.0, 10.0, 2.0, 3.0, 0.5);
        assert_close(value, 6.916_143_983_365_022, 1.0e-12);
    }

    #[test]
    fn q_sign_does_not_change_output() {
        let positive = ffq(1.25, 8.0, 2.5, 4.0, 0.7);
        let negative = ffq(-1.25, 8.0, 2.5, 4.0, 0.7);
        assert_close(positive, negative, 1.0e-12);
    }

    #[test]
    fn ef_scales_result_linearly() {
        let base = ffq(1.4, 3.0, 2.2, 2.8, 0.1);
        let doubled = ffq(1.4, 6.0, 2.2, 2.8, 0.1);
        assert_close(doubled, 2.0 * base, 1.0e-12);
    }
}
