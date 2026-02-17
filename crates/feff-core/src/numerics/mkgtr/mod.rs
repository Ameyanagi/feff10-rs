use crate::numerics::special::{lu_invert, DenseComplexMatrix, LuError};
use num_complex::Complex64;

const DEFAULT_MIN_RECIPROCAL_CONDITION: f64 = 1.0e-10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MkgtrLayout {
    pub nsp: usize,
    pub l_max: usize,
}

impl MkgtrLayout {
    pub fn new(nsp: usize, l_max: usize) -> Result<Self, MkgtrError> {
        if nsp == 0 {
            return Err(MkgtrError::InvalidSpinChannelCount { nsp });
        }

        Ok(Self { nsp, l_max })
    }

    pub fn dimension(&self) -> usize {
        self.nsp * (self.l_max + 1).pow(2)
    }

    pub fn basis_index(&self, l: usize, m: i32, spin: usize) -> Option<usize> {
        if l > self.l_max || spin >= self.nsp {
            return None;
        }

        let l_i32 = l as i32;
        if m < -l_i32 || m > l_i32 {
            return None;
        }

        let orbital_index = l_i32 * l_i32 + l_i32 + m;
        if orbital_index < 0 {
            return None;
        }

        Some(self.nsp * orbital_index as usize + spin)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MkgtrAssemblyInput<'a> {
    pub free_green: &'a DenseComplexMatrix,
    pub scattering_t: &'a [Complex64],
    pub absorptive_shift: f64,
}

impl<'a> MkgtrAssemblyInput<'a> {
    pub fn new(
        free_green: &'a DenseComplexMatrix,
        scattering_t: &'a [Complex64],
        absorptive_shift: f64,
    ) -> Self {
        Self {
            free_green,
            scattering_t,
            absorptive_shift,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MkgtrRotationInput<'a> {
    pub matrix: &'a DenseComplexMatrix,
    pub layout: MkgtrLayout,
    pub azimuth: f64,
    pub beta: f64,
    pub rotate: bool,
}

impl<'a> MkgtrRotationInput<'a> {
    pub fn new(
        matrix: &'a DenseComplexMatrix,
        layout: MkgtrLayout,
        azimuth: f64,
        beta: f64,
    ) -> Self {
        Self {
            matrix,
            layout,
            azimuth,
            beta,
            rotate: true,
        }
    }

    pub fn without_rotation(mut self) -> Self {
        self.rotate = false;
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MkgtrInversionInput<'a> {
    pub kernel: &'a DenseComplexMatrix,
    pub min_reciprocal_condition: f64,
}

impl<'a> MkgtrInversionInput<'a> {
    pub fn new(kernel: &'a DenseComplexMatrix) -> Self {
        Self {
            kernel,
            min_reciprocal_condition: DEFAULT_MIN_RECIPROCAL_CONDITION,
        }
    }

    pub fn with_min_reciprocal_condition(mut self, value: f64) -> Self {
        self.min_reciprocal_condition = value;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MkgtrInversionResult {
    pub inverse: DenseComplexMatrix,
    pub reciprocal_condition: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum MkgtrError {
    #[error("MKGTR layout requires at least one spin channel, got {nsp}")]
    InvalidSpinChannelCount { nsp: usize },
    #[error("MKGTR matrix must be square, got {rows}x{cols}")]
    NonSquareMatrix { rows: usize, cols: usize },
    #[error("MKGTR matrix dimension mismatch: expected {expected}, got {actual}")]
    MatrixDimensionMismatch { expected: usize, actual: usize },
    #[error("MKGTR scattering t-matrix length mismatch: expected {expected}, got {actual}")]
    ScatteringLengthMismatch { expected: usize, actual: usize },
    #[error("MKGTR reciprocal-condition threshold must be finite and non-negative, got {value}")]
    InvalidReciprocalConditionThreshold { value: f64 },
    #[error(
        "MKGTR kernel is ill-conditioned: reciprocal condition {reciprocal_condition:.6e} below threshold {min_reciprocal_condition:.6e}"
    )]
    IllConditionedKernel {
        reciprocal_condition: f64,
        min_reciprocal_condition: f64,
    },
    #[error("MKGTR linear solve failed: {source}")]
    LinearSolve {
        #[from]
        source: LuError,
    },
}

pub trait MkgtrKernelApi {
    fn assemble_scattering_kernel(
        &self,
        input: MkgtrAssemblyInput<'_>,
    ) -> Result<DenseComplexMatrix, MkgtrError>;

    fn rotate_green_matrix(
        &self,
        input: MkgtrRotationInput<'_>,
    ) -> Result<DenseComplexMatrix, MkgtrError>;

    fn invert_scattering_kernel(
        &self,
        input: MkgtrInversionInput<'_>,
    ) -> Result<MkgtrInversionResult, MkgtrError>;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MkgtrKernel;

impl MkgtrKernelApi for MkgtrKernel {
    fn assemble_scattering_kernel(
        &self,
        input: MkgtrAssemblyInput<'_>,
    ) -> Result<DenseComplexMatrix, MkgtrError> {
        assemble_scattering_kernel(input)
    }

    fn rotate_green_matrix(
        &self,
        input: MkgtrRotationInput<'_>,
    ) -> Result<DenseComplexMatrix, MkgtrError> {
        rotate_green_matrix(input)
    }

    fn invert_scattering_kernel(
        &self,
        input: MkgtrInversionInput<'_>,
    ) -> Result<MkgtrInversionResult, MkgtrError> {
        invert_scattering_kernel(input)
    }
}

pub fn assemble_scattering_kernel(
    input: MkgtrAssemblyInput<'_>,
) -> Result<DenseComplexMatrix, MkgtrError> {
    let dimension = validate_square_matrix(input.free_green)?;
    if input.scattering_t.len() != dimension {
        return Err(MkgtrError::ScatteringLengthMismatch {
            expected: dimension,
            actual: input.scattering_t.len(),
        });
    }

    let mut kernel = DenseComplexMatrix::zeros(dimension, dimension);
    for row in 0..dimension {
        for col in 0..dimension {
            kernel[(row, col)] = -input.free_green[(row, col)] * input.scattering_t[col];
        }

        // Mirror FEFF's complex-energy broadening convention on the diagonal.
        kernel[(row, row)] += Complex64::new(1.0, -input.absorptive_shift);
    }

    Ok(kernel)
}

pub fn rotate_green_matrix(
    input: MkgtrRotationInput<'_>,
) -> Result<DenseComplexMatrix, MkgtrError> {
    let dimension = validate_square_matrix(input.matrix)?;
    let expected_dimension = input.layout.dimension();
    if dimension != expected_dimension {
        return Err(MkgtrError::MatrixDimensionMismatch {
            expected: expected_dimension,
            actual: dimension,
        });
    }

    if !input.rotate {
        return Ok(input.matrix.clone());
    }

    let rotation_tables = build_rotation_tables(input.layout, input.azimuth, input.beta);
    let mut rotated = DenseComplexMatrix::zeros(dimension, dimension);

    for spin_col in 0..input.layout.nsp {
        for spin_row in 0..input.layout.nsp {
            for l_col in 0..=input.layout.l_max {
                let l_col_i32 = l_col as i32;
                for m_col in -l_col_i32..=l_col_i32 {
                    let col = input
                        .layout
                        .basis_index(l_col, m_col, spin_col)
                        .expect("valid MKGTR column basis index");

                    for l_row in 0..=input.layout.l_max {
                        let l_row_i32 = l_row as i32;
                        for m_row in -l_row_i32..=l_row_i32 {
                            let row = input
                                .layout
                                .basis_index(l_row, m_row, spin_row)
                                .expect("valid MKGTR row basis index");

                            let mut value = Complex64::new(0.0, 0.0);
                            for mp_col in -l_col_i32..=l_col_i32 {
                                let col_rot = input
                                    .layout
                                    .basis_index(l_col, mp_col, spin_col)
                                    .expect("valid MKGTR rotated column basis index");
                                let left = rotation_tables[l_col]
                                    [rotation_table_index(l_col, m_col, mp_col)];

                                for mp_row in -l_row_i32..=l_row_i32 {
                                    let row_rot = input
                                        .layout
                                        .basis_index(l_row, mp_row, spin_row)
                                        .expect("valid MKGTR rotated row basis index");
                                    let right = rotation_tables[l_row]
                                        [rotation_table_index(l_row, m_row, mp_row)]
                                    .conj();

                                    value += left * input.matrix[(row_rot, col_rot)] * right;
                                }
                            }

                            rotated[(row, col)] = value;
                        }
                    }
                }
            }
        }
    }

    Ok(rotated)
}

pub fn invert_scattering_kernel(
    input: MkgtrInversionInput<'_>,
) -> Result<MkgtrInversionResult, MkgtrError> {
    let _ = validate_square_matrix(input.kernel)?;

    if !input.min_reciprocal_condition.is_finite() || input.min_reciprocal_condition < 0.0 {
        return Err(MkgtrError::InvalidReciprocalConditionThreshold {
            value: input.min_reciprocal_condition,
        });
    }

    let inverse = lu_invert(input.kernel).map_err(MkgtrError::from)?;
    let kernel_norm = matrix_infinity_norm(input.kernel);
    let inverse_norm = matrix_infinity_norm(&inverse);
    let condition_number = kernel_norm * inverse_norm;

    let reciprocal_condition = if condition_number.is_finite() && condition_number > 0.0 {
        1.0 / condition_number
    } else {
        0.0
    };

    if reciprocal_condition < input.min_reciprocal_condition {
        return Err(MkgtrError::IllConditionedKernel {
            reciprocal_condition,
            min_reciprocal_condition: input.min_reciprocal_condition,
        });
    }

    Ok(MkgtrInversionResult {
        inverse,
        reciprocal_condition,
    })
}

fn validate_square_matrix(matrix: &DenseComplexMatrix) -> Result<usize, MkgtrError> {
    let rows = matrix.nrows();
    let cols = matrix.ncols();
    if rows != cols {
        return Err(MkgtrError::NonSquareMatrix { rows, cols });
    }

    Ok(rows)
}

fn build_rotation_tables(layout: MkgtrLayout, azimuth: f64, beta: f64) -> Vec<Vec<Complex64>> {
    let mut output = Vec::with_capacity(layout.l_max + 1);
    for l in 0..=layout.l_max {
        let width = 2 * l + 1;
        let mut table = vec![Complex64::new(0.0, 0.0); width * width];

        let l_i32 = l as i32;
        for m in -l_i32..=l_i32 {
            for mp in -l_i32..=l_i32 {
                table[rotation_table_index(l, m, mp)] = rotation_element(azimuth, beta, l, m, mp);
            }
        }

        output.push(table);
    }

    output
}

fn rotation_table_index(l: usize, m: i32, mp: i32) -> usize {
    let l_i32 = l as i32;
    let width = 2 * l + 1;
    let row = (m + l_i32) as usize;
    let col = (mp + l_i32) as usize;
    row * width + col
}

fn rotation_element(azimuth: f64, beta: f64, l: usize, m: i32, mp: i32) -> Complex64 {
    let phase = Complex64::from_polar(1.0, azimuth * (m as f64));
    let d_value = wigner_small_d(l, m, mp, -beta);
    phase * d_value
}

fn wigner_small_d(l: usize, m: i32, mp: i32, beta: f64) -> f64 {
    let l_i32 = l as i32;
    let k_min = (m - mp).max(0);
    let k_max = (l_i32 + m).min(l_i32 - mp);
    if k_min > k_max {
        return 0.0;
    }

    let prefactor = (factorial((l_i32 + m) as usize)
        * factorial((l_i32 - m) as usize)
        * factorial((l_i32 + mp) as usize)
        * factorial((l_i32 - mp) as usize))
    .sqrt();

    let cos_half = (0.5 * beta).cos();
    let sin_half = (0.5 * beta).sin();

    let mut sum = 0.0;
    for k in k_min..=k_max {
        let denom_1 = l_i32 + m - k;
        let denom_2 = k;
        let denom_3 = mp - m + k;
        let denom_4 = l_i32 - mp - k;

        if denom_1 < 0 || denom_2 < 0 || denom_3 < 0 || denom_4 < 0 {
            continue;
        }

        let sign = if (k + mp - m).rem_euclid(2) == 0 {
            1.0
        } else {
            -1.0
        };
        let denominator = factorial(denom_1 as usize)
            * factorial(denom_2 as usize)
            * factorial(denom_3 as usize)
            * factorial(denom_4 as usize);

        let cos_power = 2 * l_i32 + m - mp - 2 * k;
        let sin_power = mp - m + 2 * k;

        let term =
            sign * prefactor / denominator * cos_half.powi(cos_power) * sin_half.powi(sin_power);
        sum += term;
    }

    sum
}

fn factorial(value: usize) -> f64 {
    (1..=value).fold(1.0, |acc, term| acc * term as f64)
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
        assemble_scattering_kernel, invert_scattering_kernel, rotate_green_matrix,
        MkgtrAssemblyInput, MkgtrError, MkgtrInversionInput, MkgtrLayout, MkgtrRotationInput,
    };
    use crate::numerics::special::DenseComplexMatrix;
    use num_complex::Complex64;

    #[test]
    fn layout_matches_feff_channel_indexing() {
        let layout = MkgtrLayout::new(2, 2).expect("layout");

        assert_eq!(layout.dimension(), 18);
        assert_eq!(layout.basis_index(0, 0, 0), Some(0));
        assert_eq!(layout.basis_index(0, 0, 1), Some(1));
        assert_eq!(layout.basis_index(1, -1, 0), Some(2));
        assert_eq!(layout.basis_index(1, 1, 1), Some(7));
        assert_eq!(layout.basis_index(2, 2, 1), Some(17));
        assert_eq!(layout.basis_index(3, 0, 0), None);
    }

    #[test]
    fn assembly_builds_i_minus_gt_kernel() {
        let green = dense_matrix(&[
            vec![
                Complex64::new(0.20, 0.10),
                Complex64::new(-0.35, 0.05),
                Complex64::new(0.18, -0.12),
            ],
            vec![
                Complex64::new(-0.11, 0.22),
                Complex64::new(0.07, -0.05),
                Complex64::new(-0.27, 0.08),
            ],
            vec![
                Complex64::new(0.31, -0.19),
                Complex64::new(-0.09, -0.16),
                Complex64::new(0.28, 0.04),
            ],
        ]);
        let scattering = [
            Complex64::new(0.35, -0.07),
            Complex64::new(-0.22, 0.11),
            Complex64::new(0.18, 0.26),
        ];

        let kernel = assemble_scattering_kernel(MkgtrAssemblyInput::new(&green, &scattering, 0.03))
            .expect("assembly should succeed");

        let mut expected = DenseComplexMatrix::zeros(3, 3);
        for row in 0..3 {
            for col in 0..3 {
                expected[(row, col)] = -green[(row, col)] * scattering[col];
            }
            expected[(row, row)] += Complex64::new(1.0, -0.03);
        }

        assert_matrix_close(&expected, &kernel, 1.0e-12, 1.0e-12);
    }

    #[test]
    fn rotation_applies_expected_phase_at_zero_beta() {
        let layout = MkgtrLayout::new(1, 1).expect("layout");
        let mut matrix = DenseComplexMatrix::zeros(layout.dimension(), layout.dimension());

        let row = layout
            .basis_index(1, 1, 0)
            .expect("row basis index should exist");
        let col = layout
            .basis_index(1, -1, 0)
            .expect("column basis index should exist");
        let diagonal = layout
            .basis_index(1, 0, 0)
            .expect("diagonal basis index should exist");

        matrix[(row, col)] = Complex64::new(2.0, 3.0);
        matrix[(diagonal, diagonal)] = Complex64::new(1.5, -0.2);

        let azimuth = 0.4;
        let rotated = rotate_green_matrix(MkgtrRotationInput::new(&matrix, layout, azimuth, 0.0))
            .expect("rotation should succeed");

        let expected_factor = Complex64::from_polar(1.0, -2.0 * azimuth);
        let expected_off_diagonal = matrix[(row, col)] * expected_factor;

        assert_complex_close(
            "off-diagonal phase rotation",
            expected_off_diagonal,
            rotated[(row, col)],
            1.0e-11,
            1.0e-11,
        );
        assert_complex_close(
            "diagonal should remain unchanged",
            matrix[(diagonal, diagonal)],
            rotated[(diagonal, diagonal)],
            1.0e-11,
            1.0e-11,
        );
    }

    #[test]
    fn rotation_bypass_returns_original_matrix() {
        let layout = MkgtrLayout::new(1, 1).expect("layout");
        let matrix = dense_matrix(&[
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.2, -0.1),
                Complex64::new(-0.3, 0.4),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.4, 0.3),
                Complex64::new(1.2, -0.2),
                Complex64::new(0.0, 0.0),
                Complex64::new(-0.1, 0.2),
            ],
            vec![
                Complex64::new(-0.3, -0.4),
                Complex64::new(0.1, 0.2),
                Complex64::new(0.8, 0.1),
                Complex64::new(0.5, -0.2),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.1, -0.2),
                Complex64::new(-0.6, 0.3),
                Complex64::new(1.1, 0.0),
            ],
        ]);

        let rotated = rotate_green_matrix(
            MkgtrRotationInput::new(&matrix, layout, 0.7, 0.5).without_rotation(),
        )
        .expect("rotation bypass should succeed");

        assert_matrix_close(&matrix, &rotated, 1.0e-12, 1.0e-12);
    }

    #[test]
    fn inversion_recovers_identity_and_reports_conditioning() {
        let kernel = dense_matrix(&[
            vec![
                Complex64::new(2.10, -0.10),
                Complex64::new(0.35, 0.20),
                Complex64::new(-0.12, 0.18),
            ],
            vec![
                Complex64::new(-0.18, -0.05),
                Complex64::new(1.70, 0.12),
                Complex64::new(0.22, -0.16),
            ],
            vec![
                Complex64::new(0.09, 0.14),
                Complex64::new(-0.27, 0.08),
                Complex64::new(1.92, -0.07),
            ],
        ]);

        let result = invert_scattering_kernel(
            MkgtrInversionInput::new(&kernel).with_min_reciprocal_condition(1.0e-12),
        )
        .expect("inversion should succeed");

        let product = multiply(&kernel, &result.inverse);
        let identity = identity_matrix(kernel.nrows());

        assert_matrix_close(&identity, &product, 1.0e-10, 1.0e-10);
        assert!(
            result.reciprocal_condition > 1.0e-12,
            "reciprocal condition should exceed threshold"
        );
    }

    #[test]
    fn inversion_rejects_kernel_below_conditioning_threshold() {
        let kernel = dense_matrix(&[
            vec![Complex64::new(1.0, 0.0), Complex64::new(1.0, 0.0)],
            vec![Complex64::new(1.0, 0.0), Complex64::new(1.0 + 1.0e-7, 0.0)],
        ]);

        let error = invert_scattering_kernel(
            MkgtrInversionInput::new(&kernel).with_min_reciprocal_condition(1.0e-6),
        )
        .expect_err("conditioning gate should fail");

        match error {
            MkgtrError::IllConditionedKernel {
                reciprocal_condition,
                min_reciprocal_condition,
            } => {
                assert!(
                    reciprocal_condition < min_reciprocal_condition,
                    "reciprocal condition should be below threshold"
                );
                assert_eq!(min_reciprocal_condition, 1.0e-6);
            }
            other => panic!("expected IllConditionedKernel error, got {other:?}"),
        }
    }

    #[test]
    fn rotation_validates_matrix_layout_dimension() {
        let layout = MkgtrLayout::new(1, 1).expect("layout");
        let matrix = DenseComplexMatrix::zeros(2, 2);

        let error = rotate_green_matrix(MkgtrRotationInput::new(&matrix, layout, 0.0, 0.0))
            .expect_err("dimension mismatch should fail");

        match error {
            MkgtrError::MatrixDimensionMismatch { expected, actual } => {
                assert_eq!(expected, layout.dimension());
                assert_eq!(actual, 2);
            }
            other => panic!("expected MatrixDimensionMismatch error, got {other:?}"),
        }
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

    fn assert_complex_close(
        label: &str,
        expected: Complex64,
        actual: Complex64,
        abs_tol: f64,
        rel_tol: f64,
    ) {
        let diff = (expected - actual).norm();
        let scale = expected.norm().max(1.0);
        let threshold = abs_tol + rel_tol * scale;
        assert!(
            diff <= threshold,
            "{label} mismatch: expected {expected:?}, actual {actual:?}, diff={diff:.3e}, threshold={threshold:.3e}"
        );
    }
}
