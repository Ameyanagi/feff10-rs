pub fn mdff_productmatvect(matrix: [[f64; 3]; 3], input: [f64; 3]) -> [f64; 3] {
    let mut output = [0.0_f64; 3];

    for (row_index, row) in matrix.iter().enumerate() {
        output[row_index] = row
            .iter()
            .zip(input.iter())
            .map(|(matrix_value, vector_value)| matrix_value * vector_value)
            .sum();
    }

    output
}

#[cfg(test)]
mod tests {
    use super::mdff_productmatvect;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn identity_matrix_preserves_input_vector() {
        let output = mdff_productmatvect(
            [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            [2.5, -1.0, 0.125],
        );

        assert_eq!(output, [2.5, -1.0, 0.125]);
    }

    #[test]
    fn matrix_vector_product_matches_manual_reference() {
        let output = mdff_productmatvect(
            [[2.0, -1.0, 0.5], [0.0, 3.0, 1.0], [4.0, 2.0, -2.0]],
            [1.0, 2.0, -1.0],
        );

        assert_close(output[0], -0.5, 1.0e-12);
        assert_close(output[1], 5.0, 1.0e-12);
        assert_close(output[2], 10.0, 1.0e-12);
    }
}
