pub fn dist(r0: [f64; 3], r1: [f64; 3]) -> f64 {
    let dx = r0[0] - r1[0];
    let dy = r0[1] - r1[1];
    let dz = r0[2] - r1[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::dist;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn zero_distance_for_identical_points() {
        let point = [3.2, -1.0, 5.4];
        assert_close(dist(point, point), 0.0, 1.0e-12);
    }

    #[test]
    fn computes_euclidean_distance_in_three_dimensions() {
        let lhs = [0.0, 0.0, 0.0];
        let rhs = [1.0, 2.0, 2.0];
        assert_close(dist(lhs, rhs), 3.0, 1.0e-12);
    }

    #[test]
    fn distance_is_symmetric() {
        let lhs = [1.1, 2.2, 3.3];
        let rhs = [-4.4, 5.5, -6.6];
        assert_close(dist(lhs, rhs), dist(rhs, lhs), 1.0e-12);
    }
}
