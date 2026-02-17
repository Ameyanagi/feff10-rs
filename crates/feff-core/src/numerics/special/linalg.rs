use super::DenseComplexMatrix;
use num_complex::Complex64;

const SINGULAR_PIVOT_EPSILON: f64 = 1.0e-15;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LuError {
    #[error("LU factorization requires a square matrix, got {rows}x{cols}")]
    NonSquareMatrix { rows: usize, cols: usize },
    #[error("LU factorization requires a non-empty matrix")]
    EmptyMatrix,
    #[error("matrix is singular at pivot index {pivot_index}")]
    SingularMatrix { pivot_index: usize },
    #[error("right-hand side length mismatch: expected {expected}, got {actual}")]
    RhsLengthMismatch { expected: usize, actual: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LuDecomposition {
    lu: DenseComplexMatrix,
    pivots: Vec<usize>,
    pivot_sign: i32,
}

impl LuDecomposition {
    pub fn dimension(&self) -> usize {
        self.lu.nrows()
    }

    pub fn lu_matrix(&self) -> &DenseComplexMatrix {
        &self.lu
    }

    pub fn pivots(&self) -> &[usize] {
        &self.pivots
    }

    pub fn pivot_sign(&self) -> i32 {
        self.pivot_sign
    }

    pub fn solve(&self, rhs: &[Complex64]) -> Result<Vec<Complex64>, LuError> {
        let dimension = self.dimension();
        if rhs.len() != dimension {
            return Err(LuError::RhsLengthMismatch {
                expected: dimension,
                actual: rhs.len(),
            });
        }

        let mut forward = vec![Complex64::new(0.0, 0.0); dimension];
        for row in 0..dimension {
            let mut value = rhs[self.pivots[row]];
            for col in 0..row {
                value -= self.lu[(row, col)] * forward[col];
            }
            forward[row] = value;
        }

        let mut solution = vec![Complex64::new(0.0, 0.0); dimension];
        for row in (0..dimension).rev() {
            let mut value = forward[row];
            for col in (row + 1)..dimension {
                value -= self.lu[(row, col)] * solution[col];
            }

            let diagonal = self.lu[(row, row)];
            if is_effectively_zero(diagonal) {
                return Err(LuError::SingularMatrix { pivot_index: row });
            }

            solution[row] = value / diagonal;
        }

        Ok(solution)
    }
}

pub fn lu_factorize(matrix: &DenseComplexMatrix) -> Result<LuDecomposition, LuError> {
    let dimension = validate_square_shape(matrix)?;
    let mut lu = matrix.clone();
    let mut pivots: Vec<usize> = (0..dimension).collect();
    let mut pivot_sign = 1;
    let pivot_threshold_sq = SINGULAR_PIVOT_EPSILON * SINGULAR_PIVOT_EPSILON;

    for pivot_col in 0..dimension {
        let (pivot_row, pivot_norm_sq) = select_pivot_row(&lu, pivot_col);
        if pivot_norm_sq <= pivot_threshold_sq {
            return Err(LuError::SingularMatrix {
                pivot_index: pivot_col,
            });
        }

        if pivot_row != pivot_col {
            swap_rows(&mut lu, pivot_col, pivot_row);
            pivots.swap(pivot_col, pivot_row);
            pivot_sign = -pivot_sign;
        }

        let pivot = lu[(pivot_col, pivot_col)];
        if is_effectively_zero(pivot) {
            return Err(LuError::SingularMatrix {
                pivot_index: pivot_col,
            });
        }

        for row in (pivot_col + 1)..dimension {
            lu[(row, pivot_col)] /= pivot;
            let multiplier = lu[(row, pivot_col)];
            for col in (pivot_col + 1)..dimension {
                let updated = lu[(row, col)] - multiplier * lu[(pivot_col, col)];
                lu[(row, col)] = updated;
            }
        }
    }

    Ok(LuDecomposition {
        lu,
        pivots,
        pivot_sign,
    })
}

pub fn lu_solve(matrix: &DenseComplexMatrix, rhs: &[Complex64]) -> Result<Vec<Complex64>, LuError> {
    lu_factorize(matrix)?.solve(rhs)
}

pub trait LuLinearSolveApi {
    fn lu_factorize(&self, matrix: &DenseComplexMatrix) -> Result<LuDecomposition, LuError>;
    fn lu_solve(
        &self,
        matrix: &DenseComplexMatrix,
        rhs: &[Complex64],
    ) -> Result<Vec<Complex64>, LuError>;
}

fn validate_square_shape(matrix: &DenseComplexMatrix) -> Result<usize, LuError> {
    let rows = matrix.nrows();
    let cols = matrix.ncols();
    if rows == 0 || cols == 0 {
        return Err(LuError::EmptyMatrix);
    }
    if rows != cols {
        return Err(LuError::NonSquareMatrix { rows, cols });
    }

    Ok(rows)
}

fn select_pivot_row(matrix: &DenseComplexMatrix, pivot_col: usize) -> (usize, f64) {
    let dimension = matrix.nrows();
    let mut best_row = pivot_col;
    let mut best_norm_sq = matrix[(pivot_col, pivot_col)].norm_sqr();

    for row in (pivot_col + 1)..dimension {
        let norm_sq = matrix[(row, pivot_col)].norm_sqr();
        if norm_sq > best_norm_sq {
            best_norm_sq = norm_sq;
            best_row = row;
        }
    }

    (best_row, best_norm_sq)
}

fn swap_rows(matrix: &mut DenseComplexMatrix, lhs: usize, rhs: usize) {
    if lhs == rhs {
        return;
    }

    for col in 0..matrix.ncols() {
        let value = matrix[(lhs, col)];
        matrix[(lhs, col)] = matrix[(rhs, col)];
        matrix[(rhs, col)] = value;
    }
}

fn is_effectively_zero(value: Complex64) -> bool {
    value.norm_sqr() <= SINGULAR_PIVOT_EPSILON * SINGULAR_PIVOT_EPSILON
}

#[cfg(test)]
mod tests {
    use super::{lu_factorize, lu_solve, LuError};
    use crate::numerics::special::DenseComplexMatrix;
    use num_complex::Complex64;

    #[test]
    fn lu_factorize_reconstructs_permuted_original_matrix() {
        let matrix = dense_matrix(&[
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(2.0, -1.0),
                Complex64::new(1.0, 0.0),
            ],
            vec![
                Complex64::new(1.0, 2.0),
                Complex64::new(-2.0, 0.5),
                Complex64::new(-3.0, -1.0),
            ],
            vec![
                Complex64::new(2.0, -1.0),
                Complex64::new(3.0, 0.0),
                Complex64::new(1.0, 4.0),
            ],
        ]);
        let decomposition = lu_factorize(&matrix).expect("LU decomposition");

        let permuted = permute_rows(&matrix, decomposition.pivots());
        let (l, u) = split_lu(decomposition.lu_matrix());
        let recomposed = multiply(&l, &u);

        assert_matrix_close(&permuted, &recomposed, 1.0e-12, 1.0e-12);
    }

    #[test]
    fn lu_solve_recovers_known_complex_solution() {
        let matrix = dense_matrix(&[
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(2.0, -1.0),
                Complex64::new(1.0, 0.0),
            ],
            vec![
                Complex64::new(1.0, 2.0),
                Complex64::new(-2.0, 0.5),
                Complex64::new(-3.0, -1.0),
            ],
            vec![
                Complex64::new(2.0, -1.0),
                Complex64::new(3.0, 0.0),
                Complex64::new(1.0, 4.0),
            ],
        ]);
        let expected = vec![
            Complex64::new(1.0, -1.0),
            Complex64::new(2.0, 0.5),
            Complex64::new(-0.5, 2.0),
        ];
        let rhs = matvec(&matrix, &expected);

        let actual = lu_solve(&matrix, &rhs).expect("solve");
        assert_vector_close(&expected, &actual, 1.0e-12, 1.0e-12);
    }

    #[test]
    fn lu_factorize_rejects_non_square_matrices() {
        let matrix = DenseComplexMatrix::zeros(2, 3);
        let error = lu_factorize(&matrix).expect_err("non-square matrix should fail");
        assert_eq!(error, LuError::NonSquareMatrix { rows: 2, cols: 3 });
    }

    #[test]
    fn lu_factorize_rejects_singular_matrices() {
        let matrix = dense_matrix(&[
            vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
            vec![Complex64::new(2.0, 0.0), Complex64::new(4.0, 0.0)],
        ]);
        let error = lu_factorize(&matrix).expect_err("singular matrix should fail");
        assert_eq!(error, LuError::SingularMatrix { pivot_index: 1 });
    }

    #[test]
    fn lu_solve_validates_rhs_dimension() {
        let matrix = dense_matrix(&[
            vec![Complex64::new(3.0, 0.0), Complex64::new(1.0, 0.0)],
            vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
        ]);
        let decomposition = lu_factorize(&matrix).expect("decomposition");

        let error = decomposition
            .solve(&[Complex64::new(1.0, 0.0)])
            .expect_err("rhs mismatch should fail");
        assert_eq!(
            error,
            LuError::RhsLengthMismatch {
                expected: 2,
                actual: 1
            }
        );
    }

    fn dense_matrix(rows: &[Vec<Complex64>]) -> DenseComplexMatrix {
        let nrows = rows.len();
        let ncols = rows.first().map_or(0, |row| row.len());
        assert!(
            rows.iter().all(|row| row.len() == ncols),
            "all matrix rows must have the same width"
        );

        let mut matrix = DenseComplexMatrix::zeros(nrows, ncols);
        for (row_index, row) in rows.iter().enumerate() {
            for (col_index, value) in row.iter().enumerate() {
                matrix[(row_index, col_index)] = *value;
            }
        }
        matrix
    }

    fn permute_rows(matrix: &DenseComplexMatrix, pivots: &[usize]) -> DenseComplexMatrix {
        let nrows = matrix.nrows();
        let ncols = matrix.ncols();
        assert_eq!(pivots.len(), nrows, "pivot count must match matrix size");

        let mut permuted = DenseComplexMatrix::zeros(nrows, ncols);
        for row in 0..nrows {
            let source = pivots[row];
            for col in 0..ncols {
                permuted[(row, col)] = matrix[(source, col)];
            }
        }
        permuted
    }

    fn split_lu(packed: &DenseComplexMatrix) -> (DenseComplexMatrix, DenseComplexMatrix) {
        let nrows = packed.nrows();
        let ncols = packed.ncols();
        let mut lower = DenseComplexMatrix::zeros(nrows, ncols);
        let mut upper = DenseComplexMatrix::zeros(nrows, ncols);

        for row in 0..nrows {
            for col in 0..ncols {
                if row > col {
                    lower[(row, col)] = packed[(row, col)];
                } else if row == col {
                    lower[(row, col)] = Complex64::new(1.0, 0.0);
                    upper[(row, col)] = packed[(row, col)];
                } else {
                    upper[(row, col)] = packed[(row, col)];
                }
            }
        }

        (lower, upper)
    }

    fn multiply(lhs: &DenseComplexMatrix, rhs: &DenseComplexMatrix) -> DenseComplexMatrix {
        let nrows = lhs.nrows();
        let inner = lhs.ncols();
        let ncols = rhs.ncols();
        assert_eq!(rhs.nrows(), inner, "inner matrix dimensions must match");

        let mut output = DenseComplexMatrix::zeros(nrows, ncols);
        for row in 0..nrows {
            for col in 0..ncols {
                let mut sum = Complex64::new(0.0, 0.0);
                for k in 0..inner {
                    sum += lhs[(row, k)] * rhs[(k, col)];
                }
                output[(row, col)] = sum;
            }
        }
        output
    }

    fn matvec(matrix: &DenseComplexMatrix, vector: &[Complex64]) -> Vec<Complex64> {
        let nrows = matrix.nrows();
        let ncols = matrix.ncols();
        assert_eq!(
            vector.len(),
            ncols,
            "vector length must match matrix columns"
        );

        let mut output = vec![Complex64::new(0.0, 0.0); nrows];
        for row in 0..nrows {
            let mut sum = Complex64::new(0.0, 0.0);
            for col in 0..ncols {
                sum += matrix[(row, col)] * vector[col];
            }
            output[row] = sum;
        }
        output
    }

    fn assert_matrix_close(
        expected: &DenseComplexMatrix,
        actual: &DenseComplexMatrix,
        abs_tol: f64,
        rel_tol: f64,
    ) {
        assert_eq!(expected.nrows(), actual.nrows(), "row count mismatch");
        assert_eq!(expected.ncols(), actual.ncols(), "column count mismatch");

        for row in 0..expected.nrows() {
            for col in 0..expected.ncols() {
                assert_complex_close(
                    &format!("entry ({row},{col})"),
                    expected[(row, col)],
                    actual[(row, col)],
                    abs_tol,
                    rel_tol,
                );
            }
        }
    }

    fn assert_vector_close(
        expected: &[Complex64],
        actual: &[Complex64],
        abs_tol: f64,
        rel_tol: f64,
    ) {
        assert_eq!(expected.len(), actual.len(), "vector length mismatch");
        for (index, (&expected_value, &actual_value)) in expected.iter().zip(actual).enumerate() {
            assert_complex_close(
                &format!("entry {index}"),
                expected_value,
                actual_value,
                abs_tol,
                rel_tol,
            );
        }
    }

    fn assert_complex_close(
        label: &str,
        expected: Complex64,
        actual: Complex64,
        abs_tol: f64,
        rel_tol: f64,
    ) {
        let abs_diff = (actual - expected).norm();
        let rel_diff = abs_diff / expected.norm().max(1.0);
        assert!(
            abs_diff <= abs_tol || rel_diff <= rel_tol,
            "{label} expected=({:.15e},{:.15e}) actual=({:.15e},{:.15e}) abs_diff={:.15e} rel_diff={:.15e} abs_tol={:.15e} rel_tol={:.15e}",
            expected.re,
            expected.im,
            actual.re,
            actual.im,
            abs_diff,
            rel_diff,
            abs_tol,
            rel_tol
        );
    }
}
