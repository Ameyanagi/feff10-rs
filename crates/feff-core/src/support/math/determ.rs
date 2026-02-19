pub fn determ(array: &mut [Vec<f64>], nord: usize) -> Option<f64> {
    if nord == 0 {
        return Some(1.0);
    }

    if array.len() < nord || array.iter().take(nord).any(|row| row.len() < nord) {
        return None;
    }

    let mut determinant = 1.0f64;

    for k in 0..nord {
        if array[k][k] == 0.0 {
            let pivot_col = (k..nord).find(|&j| array[k][j] != 0.0);
            let Some(pivot_col) = pivot_col else {
                return Some(0.0);
            };

            for row in array.iter_mut().take(nord).skip(k) {
                row.swap(k, pivot_col);
            }
            determinant = -determinant;
        }

        determinant *= array[k][k];
        if k + 1 >= nord {
            continue;
        }

        let pivot = array[k][k];
        for i in (k + 1)..nord {
            for j in (k + 1)..nord {
                array[i][j] -= array[i][k] * array[k][j] / pivot;
            }
        }
    }

    Some(determinant)
}

#[cfg(test)]
mod tests {
    use super::determ;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn computes_two_by_two_determinant() {
        let mut matrix = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let value = determ(&mut matrix, 2).expect("valid matrix");
        assert_close(value, -2.0, 1.0e-12);
    }

    #[test]
    fn column_swap_path_preserves_sign_convention() {
        let mut matrix = vec![vec![0.0, 2.0], vec![3.0, 4.0]];
        let value = determ(&mut matrix, 2).expect("valid matrix");
        assert_close(value, -6.0, 1.0e-12);
    }

    #[test]
    fn singular_matrix_returns_zero() {
        let mut matrix = vec![vec![1.0, 2.0], vec![2.0, 4.0]];
        let value = determ(&mut matrix, 2).expect("valid matrix");
        assert_close(value, 0.0, 1.0e-12);
    }

    #[test]
    fn invalid_dimensions_return_none() {
        let mut matrix = vec![vec![1.0, 2.0]];
        assert_eq!(determ(&mut matrix, 2), None);
    }
}
