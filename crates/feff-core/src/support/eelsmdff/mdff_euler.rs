pub fn mdff_euler(alpha: f64, beta: f64, gamma: f64) -> [[f64; 3]; 3] {
    let mut e = [[0.0_f64; 3]; 3];

    e[0][0] = alpha.cos() * beta.cos() * gamma.cos() - alpha.sin() * gamma.sin();
    e[1][0] = alpha.sin() * beta.cos() * gamma.cos() + alpha.cos() * gamma.sin();
    e[0][1] = -alpha.cos() * beta.cos() * gamma.sin() - alpha.sin() * gamma.cos();
    e[1][1] = -alpha.sin() * beta.cos() * gamma.sin() + alpha.cos() * gamma.cos();
    e[0][2] = alpha.cos() * beta.sin();
    e[1][2] = alpha.sin() * beta.sin();
    e[2][2] = beta.cos();
    e[2][0] = -beta.sin() * gamma.cos();
    e[2][1] = beta.sin() * gamma.sin();

    e
}

pub fn mdff_determinant(matrix: [[f64; 3]; 3]) -> f64 {
    matrix[0][0] * matrix[1][1] * matrix[2][2]
        + matrix[0][1] * matrix[1][2] * matrix[2][0]
        + matrix[1][0] * matrix[2][1] * matrix[0][2]
        - matrix[2][0] * matrix[1][1] * matrix[0][2]
        - matrix[1][0] * matrix[0][1] * matrix[2][2]
        - matrix[0][0] * matrix[2][1] * matrix[1][2]
}

pub fn is_proper_euler_matrix(matrix: [[f64; 3]; 3], tolerance: f64) -> bool {
    (mdff_determinant(matrix) - 1.0).abs() <= tolerance.abs()
}

#[cfg(test)]
mod tests {
    use super::{is_proper_euler_matrix, mdff_determinant, mdff_euler};
    use std::f64::consts::FRAC_PI_2;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn zero_angles_return_identity_matrix() {
        let matrix = mdff_euler(0.0, 0.0, 0.0);

        assert_close(matrix[0][0], 1.0, 1.0e-12);
        assert_close(matrix[1][1], 1.0, 1.0e-12);
        assert_close(matrix[2][2], 1.0, 1.0e-12);
        assert_close(matrix[0][1], 0.0, 1.0e-12);
        assert_close(matrix[1][0], 0.0, 1.0e-12);
        assert_close(matrix[2][0], 0.0, 1.0e-12);
    }

    #[test]
    fn determinant_matches_known_matrix_value() {
        let matrix = [[1.0, 2.0, 3.0], [0.5, -1.0, 0.0], [2.0, 1.0, 4.0]];
        let det = mdff_determinant(matrix);
        assert_close(det, -0.5, 1.0e-12);
    }

    #[test]
    fn euler_rotation_has_unit_determinant() {
        let matrix = mdff_euler(0.3, 0.9, -0.4);
        assert!(is_proper_euler_matrix(matrix, 1.0e-10));
    }

    #[test]
    fn right_angle_rotation_matches_expected_axes() {
        let matrix = mdff_euler(0.0, FRAC_PI_2, 0.0);

        assert_close(matrix[0][2], 1.0, 1.0e-12);
        assert_close(matrix[2][0], -1.0, 1.0e-12);
        assert_close(matrix[1][1], 1.0, 1.0e-12);
    }
}
