use super::DenseComplexMatrix;
use num_complex::Complex64;

const SINGULAR_PIVOT_EPSILON: f64 = 1.0e-15;
const ILL_CONDITIONED_RELATIVE_PIVOT_EPSILON: f64 = 1.0e-12;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LuError {
    #[error("LU factorization requires a square matrix, got {rows}x{cols}")]
    NonSquareMatrix { rows: usize, cols: usize },
    #[error("LU factorization requires a non-empty matrix")]
    EmptyMatrix,
    #[error("matrix is singular at pivot index {pivot_index}")]
    SingularMatrix { pivot_index: usize },
    #[error("matrix is ill-conditioned at pivot index {pivot_index}")]
    IllConditionedMatrix { pivot_index: usize },
    #[error("right-hand side length mismatch: expected {expected}, got {actual}")]
    RhsLengthMismatch { expected: usize, actual: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LuDecomposition {
    lu: DenseComplexMatrix,
    pivots: Vec<usize>,
    pivot_sign: i32,
    input_norm_infty: f64,
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

    pub fn invert(&self) -> Result<DenseComplexMatrix, LuError> {
        let dimension = self.dimension();
        let mut inverse = DenseComplexMatrix::zeros(dimension, dimension);
        let mut basis = vec![Complex64::new(0.0, 0.0); dimension];

        for pivot_index in 0..dimension {
            let diagonal = self.lu[(pivot_index, pivot_index)];
            if is_ill_conditioned_pivot(diagonal, self.input_norm_infty) {
                return Err(LuError::IllConditionedMatrix { pivot_index });
            }
        }

        for col in 0..dimension {
            basis.fill(Complex64::new(0.0, 0.0));
            basis[col] = Complex64::new(1.0, 0.0);

            let solution = self.solve(&basis)?;
            for row in 0..dimension {
                inverse[(row, col)] = solution[row];
            }
        }

        Ok(inverse)
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
    let input_norm_infty = matrix_infinity_norm(matrix);
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
        input_norm_infty,
    })
}

pub fn lu_solve(matrix: &DenseComplexMatrix, rhs: &[Complex64]) -> Result<Vec<Complex64>, LuError> {
    lu_factorize(matrix)?.solve(rhs)
}

pub fn lu_invert(matrix: &DenseComplexMatrix) -> Result<DenseComplexMatrix, LuError> {
    lu_factorize(matrix)?.invert()
}

pub trait LuLinearSolveApi {
    fn lu_factorize(&self, matrix: &DenseComplexMatrix) -> Result<LuDecomposition, LuError>;
    fn lu_solve(
        &self,
        matrix: &DenseComplexMatrix,
        rhs: &[Complex64],
    ) -> Result<Vec<Complex64>, LuError>;
    fn lu_invert(&self, matrix: &DenseComplexMatrix) -> Result<DenseComplexMatrix, LuError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EigenError {
    #[error("eigendecomposition requires a square matrix, got {rows}x{cols}")]
    NonSquareMatrix { rows: usize, cols: usize },
    #[error("eigendecomposition requires a non-empty matrix")]
    EmptyMatrix,
    #[error("eigendecomposition did not converge")]
    NoConvergence,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EigenDecomposition {
    eigenvectors: DenseComplexMatrix,
    eigenvalues: Vec<Complex64>,
}

impl EigenDecomposition {
    pub fn dimension(&self) -> usize {
        self.eigenvalues.len()
    }

    pub fn eigenvectors(&self) -> &DenseComplexMatrix {
        &self.eigenvectors
    }

    pub fn eigenvalues(&self) -> &[Complex64] {
        &self.eigenvalues
    }
}

pub fn eigen_decompose(matrix: &DenseComplexMatrix) -> Result<EigenDecomposition, EigenError> {
    let dimension = validate_square_shape_for_eigen(matrix)?;
    let faer_eigen = matrix.eigen().map_err(|_| EigenError::NoConvergence)?;
    let mut eigenvectors = DenseComplexMatrix::zeros(dimension, dimension);
    let mut eigenvalues = vec![Complex64::new(0.0, 0.0); dimension];

    for index in 0..dimension {
        eigenvalues[index] = faer_eigen.S()[index];
    }

    for row in 0..dimension {
        for col in 0..dimension {
            eigenvectors[(row, col)] = faer_eigen.U()[(row, col)];
        }
    }

    sort_eigenpairs(&mut eigenvalues, &mut eigenvectors);

    Ok(EigenDecomposition {
        eigenvectors,
        eigenvalues,
    })
}

pub fn eigenvalues(matrix: &DenseComplexMatrix) -> Result<Vec<Complex64>, EigenError> {
    Ok(eigen_decompose(matrix)?.eigenvalues)
}

pub trait EigenvalueSolveApi {
    fn eigen_decompose(
        &self,
        matrix: &DenseComplexMatrix,
    ) -> Result<EigenDecomposition, EigenError>;
    fn eigenvalues(&self, matrix: &DenseComplexMatrix) -> Result<Vec<Complex64>, EigenError>;
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

fn validate_square_shape_for_eigen(matrix: &DenseComplexMatrix) -> Result<usize, EigenError> {
    let rows = matrix.nrows();
    let cols = matrix.ncols();
    if rows == 0 || cols == 0 {
        return Err(EigenError::EmptyMatrix);
    }
    if rows != cols {
        return Err(EigenError::NonSquareMatrix { rows, cols });
    }

    Ok(rows)
}

fn sort_eigenpairs(eigenvalues: &mut [Complex64], eigenvectors: &mut DenseComplexMatrix) {
    let dimension = eigenvalues.len();
    let mut order: Vec<usize> = (0..dimension).collect();
    order.sort_unstable_by(|lhs, rhs| {
        eigenvalues[*lhs]
            .re
            .total_cmp(&eigenvalues[*rhs].re)
            .then_with(|| eigenvalues[*lhs].im.total_cmp(&eigenvalues[*rhs].im))
    });

    if order
        .iter()
        .enumerate()
        .all(|(index, &entry)| index == entry)
    {
        return;
    }

    let unsorted_values = eigenvalues.to_vec();
    let mut sorted_vectors = DenseComplexMatrix::zeros(dimension, dimension);
    for (target_col, &source_col) in order.iter().enumerate() {
        eigenvalues[target_col] = unsorted_values[source_col];
        for row in 0..dimension {
            sorted_vectors[(row, target_col)] = eigenvectors[(row, source_col)];
        }
    }

    *eigenvectors = sorted_vectors;
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

fn is_ill_conditioned_pivot(pivot: Complex64, input_norm_infty: f64) -> bool {
    pivot.norm() <= input_norm_infty * ILL_CONDITIONED_RELATIVE_PIVOT_EPSILON
}

fn matrix_infinity_norm(matrix: &DenseComplexMatrix) -> f64 {
    let mut best_row_sum: f64 = 0.0;
    for row in 0..matrix.nrows() {
        let mut row_sum = 0.0;
        for col in 0..matrix.ncols() {
            row_sum += matrix[(row, col)].norm();
        }
        best_row_sum = best_row_sum.max(row_sum);
    }
    best_row_sum
}

#[cfg(test)]
mod tests {
    use super::{
        eigen_decompose, eigenvalues, lu_factorize, lu_invert, lu_solve, EigenError, LuError,
    };
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

    #[test]
    fn lu_invert_recovers_identity_when_recomposed() {
        let matrix = dense_matrix(&[
            vec![
                Complex64::new(1.5, 0.0),
                Complex64::new(-2.0, 1.0),
                Complex64::new(0.5, -0.5),
            ],
            vec![
                Complex64::new(0.75, 2.0),
                Complex64::new(3.0, -1.0),
                Complex64::new(-1.0, 0.25),
            ],
            vec![
                Complex64::new(2.0, -0.5),
                Complex64::new(1.25, 0.0),
                Complex64::new(2.5, 1.5),
            ],
        ]);

        let inverse = lu_invert(&matrix).expect("inverse");
        let product = multiply(&matrix, &inverse);
        let identity = identity_matrix(matrix.nrows());

        assert_matrix_close(&identity, &product, 1.0e-10, 1.0e-10);
    }

    #[test]
    fn lu_invert_rejects_singular_matrices() {
        let matrix = dense_matrix(&[
            vec![Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
            vec![Complex64::new(2.0, 0.0), Complex64::new(4.0, 0.0)],
        ]);

        let error = lu_invert(&matrix).expect_err("singular matrix should fail");
        assert_eq!(error, LuError::SingularMatrix { pivot_index: 1 });
    }

    #[test]
    fn lu_invert_rejects_ill_conditioned_matrices() {
        let matrix = dense_matrix(&[
            vec![Complex64::new(1.0, 0.0), Complex64::new(1.0, 0.0)],
            vec![Complex64::new(1.0, 0.0), Complex64::new(1.0 + 1.0e-14, 0.0)],
        ]);

        let error = lu_invert(&matrix).expect_err("ill-conditioned matrix should fail");
        assert_eq!(error, LuError::IllConditionedMatrix { pivot_index: 1 });
    }

    #[test]
    fn eigen_decompose_rejects_non_square_matrices() {
        let matrix = DenseComplexMatrix::zeros(2, 3);
        let error = eigen_decompose(&matrix).expect_err("non-square matrix should fail");
        assert_eq!(error, EigenError::NonSquareMatrix { rows: 2, cols: 3 });
    }

    #[test]
    fn eigen_decompose_rejects_empty_matrices() {
        let matrix = DenseComplexMatrix::zeros(0, 0);
        let error = eigen_decompose(&matrix).expect_err("empty matrix should fail");
        assert_eq!(error, EigenError::EmptyMatrix);
    }

    #[test]
    fn eigen_helpers_match_upper_triangular_scattering_reference_spectrum() {
        let matrix = dense_matrix(&[
            vec![
                Complex64::new(1.60, -0.20),
                Complex64::new(0.12, 0.04),
                Complex64::new(-0.08, 0.03),
            ],
            vec![
                Complex64::new(0.00, 0.00),
                Complex64::new(0.85, 0.15),
                Complex64::new(0.22, -0.11),
            ],
            vec![
                Complex64::new(0.00, 0.00),
                Complex64::new(0.00, 0.00),
                Complex64::new(-0.35, 0.05),
            ],
        ]);

        let decomposition = eigen_decompose(&matrix).expect("eigendecomposition");
        assert_eigenpair_residuals(
            &matrix,
            decomposition.eigenvectors(),
            decomposition.eigenvalues(),
        );

        let expected = vec![
            Complex64::new(-0.35, 0.05),
            Complex64::new(0.85, 0.15),
            Complex64::new(1.60, -0.20),
        ];
        assert_vector_close(&expected, decomposition.eigenvalues(), 1.0e-11, 1.0e-10);

        let direct_values = eigenvalues(&matrix).expect("eigenvalues");
        assert_vector_close(&expected, &direct_values, 1.0e-11, 1.0e-10);
    }

    #[test]
    fn eigen_decompose_validates_dense_scattering_eigenpairs() {
        let matrix = dense_matrix(&[
            vec![
                Complex64::new(1.05, 0.12),
                Complex64::new(-0.31, 0.08),
                Complex64::new(0.18, -0.06),
            ],
            vec![
                Complex64::new(0.27, -0.14),
                Complex64::new(0.72, 0.21),
                Complex64::new(-0.43, 0.05),
            ],
            vec![
                Complex64::new(-0.16, 0.11),
                Complex64::new(0.39, -0.18),
                Complex64::new(1.34, -0.07),
            ],
        ]);

        let decomposition = eigen_decompose(&matrix).expect("eigendecomposition");
        assert_eigenpair_residuals(
            &matrix,
            decomposition.eigenvectors(),
            decomposition.eigenvalues(),
        );

        let trace = matrix[(0, 0)] + matrix[(1, 1)] + matrix[(2, 2)];
        let spectral_trace = decomposition
            .eigenvalues()
            .iter()
            .copied()
            .fold(Complex64::new(0.0, 0.0), |sum, value| sum + value);
        assert_complex_close("trace consistency", trace, spectral_trace, 1.0e-10, 1.0e-9);
    }

    #[test]
    fn eigenvalues_capture_expected_complex_pair_for_real_kernel() {
        let matrix = dense_matrix(&[
            vec![Complex64::new(0.6, 0.0), Complex64::new(-2.0, 0.0)],
            vec![Complex64::new(0.5, 0.0), Complex64::new(0.6, 0.0)],
        ]);

        let values = eigenvalues(&matrix).expect("eigenvalues");
        let expected = vec![Complex64::new(0.6, -1.0), Complex64::new(0.6, 1.0)];
        assert_vector_close(&expected, &values, 1.0e-10, 1.0e-9);

        let decomposition = eigen_decompose(&matrix).expect("eigendecomposition");
        assert_eigenpair_residuals(
            &matrix,
            decomposition.eigenvectors(),
            decomposition.eigenvalues(),
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

    fn identity_matrix(size: usize) -> DenseComplexMatrix {
        let mut identity = DenseComplexMatrix::zeros(size, size);
        for index in 0..size {
            identity[(index, index)] = Complex64::new(1.0, 0.0);
        }
        identity
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

    fn assert_eigenpair_residuals(
        matrix: &DenseComplexMatrix,
        eigenvectors: &DenseComplexMatrix,
        eigenvalues: &[Complex64],
    ) {
        assert_eq!(matrix.nrows(), matrix.ncols(), "matrix must be square");
        assert_eq!(eigenvectors.nrows(), matrix.nrows(), "row count mismatch");
        assert_eq!(
            eigenvectors.ncols(),
            matrix.ncols(),
            "column count mismatch"
        );
        assert_eq!(
            eigenvalues.len(),
            matrix.nrows(),
            "eigenvalue count mismatch"
        );

        let dimension = matrix.nrows();
        for col in 0..dimension {
            let lambda = eigenvalues[col];
            let mut residual_norm_sq = 0.0;
            let mut scale_norm_sq = 0.0;

            for row in 0..dimension {
                let mut lhs = Complex64::new(0.0, 0.0);
                for k in 0..dimension {
                    lhs += matrix[(row, k)] * eigenvectors[(k, col)];
                }
                let rhs = lambda * eigenvectors[(row, col)];
                let residual = lhs - rhs;

                residual_norm_sq += residual.norm_sqr();
                scale_norm_sq += lhs.norm_sqr().max(rhs.norm_sqr());
            }

            let residual_norm = residual_norm_sq.sqrt();
            let relative_residual = residual_norm / scale_norm_sq.sqrt().max(1.0);
            assert!(
                residual_norm <= 1.0e-10 || relative_residual <= 1.0e-9,
                "eigenpair {col} residual too large: abs={residual_norm:.3e}, rel={relative_residual:.3e}"
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
