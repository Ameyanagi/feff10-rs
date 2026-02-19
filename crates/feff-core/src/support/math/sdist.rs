use super::dist::dist;

pub fn sdist(r0: [f64; 3], r1: [f64; 3]) -> f64 {
    dist(r0, r1)
}

#[cfg(test)]
mod tests {
    use super::sdist;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn computes_distance_between_cartesian_points() {
        let lhs = [0.0, 0.0, 0.0];
        let rhs = [1.0, 2.0, 2.0];
        assert_close(sdist(lhs, rhs), 3.0, 1.0e-12);
    }

    #[test]
    fn returns_zero_for_identical_points() {
        let point = [0.4, -1.6, 8.2];
        assert_close(sdist(point, point), 0.0, 1.0e-12);
    }
}
