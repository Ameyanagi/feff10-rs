use crate::support::math::rotwig::{RotwigError, rotwig};
use num_complex::Complex64;

#[derive(Debug, thiserror::Error)]
pub enum RotgMatrixError {
    #[error("matrix must be {expected}x{expected}, got {rows}x{cols}")]
    DimensionMismatch {
        expected: usize,
        rows: usize,
        cols: usize,
    },
    #[error("invalid nsp={0}; expected at least 1")]
    InvalidSpinChannels(usize),
    #[error(transparent)]
    Rotwig(#[from] RotwigError),
}

pub fn gmatrix_dimension(nsp: usize, lx: usize) -> usize {
    nsp * (lx + 1).pow(2)
}

pub fn rotgmatrix(
    nq: usize,
    elpty: f64,
    pha: Complex64,
    beta: f64,
    nsp: usize,
    lx: usize,
    gg: &[Vec<Complex64>],
) -> Result<Vec<Vec<Complex64>>, RotgMatrixError> {
    if nsp == 0 {
        return Err(RotgMatrixError::InvalidSpinChannels(nsp));
    }

    let expected = gmatrix_dimension(nsp, lx);
    let rows = gg.len();
    let cols = gg.first().map_or(0, Vec::len);
    if rows != expected || gg.iter().any(|row| row.len() != expected) {
        return Err(RotgMatrixError::DimensionMismatch {
            expected,
            rows,
            cols,
        });
    }

    if nq == 0 || elpty < 0.0 {
        return Ok(gg.to_vec());
    }

    let mut rotm = vec![vec![vec![Complex64::new(0.0, 0.0); 2 * lx + 1]; 2 * lx + 1]; lx + 1];

    for l in 0..=lx {
        for m1 in -(l as i32)..=(l as i32) {
            for m2 in -(l as i32)..=(l as i32) {
                let pham = pha.conj().powi(m2);
                let rotation = rotwig(-beta, l as i32, m2, m1, 1)?;
                rotm[l][(m2 + l as i32) as usize][(m1 + l as i32) as usize] = pham * rotation;
            }
        }
    }

    let mut ggrot = vec![vec![Complex64::new(0.0, 0.0); expected]; expected];

    for is1 in 0..nsp {
        for is2 in 0..nsp {
            for l1 in 0..=lx {
                for m1 in -(l1 as i32)..=(l1 as i32) {
                    let ig1 = lm_spin_index(l1, m1, is1, nsp);
                    for l2 in 0..=lx {
                        for m2 in -(l2 as i32)..=(l2 as i32) {
                            let ig2 = lm_spin_index(l2, m2, is2, nsp);
                            let mut accum = Complex64::new(0.0, 0.0);

                            for mp1 in -(l1 as i32)..=(l1 as i32) {
                                let igp1 = lm_spin_index(l1, mp1, is1, nsp);
                                for mp2 in -(l2 as i32)..=(l2 as i32) {
                                    let igp2 = lm_spin_index(l2, mp2, is2, nsp);
                                    accum += rotm[l1][(m1 + l1 as i32) as usize]
                                        [(mp1 + l1 as i32) as usize]
                                        * gg[igp1][igp2]
                                        * rotm[l2][(m2 + l2 as i32) as usize]
                                            [(mp2 + l2 as i32) as usize]
                                            .conj();
                                }
                            }

                            ggrot[ig2][ig1] = accum;
                        }
                    }
                }
            }
        }
    }

    Ok(ggrot)
}

fn lm_spin_index(l: usize, m: i32, spin: usize, nsp: usize) -> usize {
    let idx_1_based = (nsp as i32) * (l * l + l) as i32 + (nsp as i32) * m + (spin as i32 + 1);
    (idx_1_based - 1) as usize
}

#[cfg(test)]
mod tests {
    use super::{gmatrix_dimension, rotgmatrix};
    use num_complex::Complex64;

    fn assert_close(actual: Complex64, expected: Complex64, tolerance: f64) {
        assert!(
            (actual - expected).norm() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn spherical_or_averaged_path_returns_unrotated_matrix() {
        let nsp = 1;
        let lx = 1;
        let dimension = gmatrix_dimension(nsp, lx);
        let gg = vec![
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.1, 0.2),
                Complex64::new(0.0, 0.0),
                Complex64::new(-0.2, 0.1),
            ],
            vec![
                Complex64::new(0.1, -0.2),
                Complex64::new(1.5, 0.0),
                Complex64::new(0.3, 0.1),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.3, -0.1),
                Complex64::new(1.2, 0.0),
                Complex64::new(0.1, 0.0),
            ],
            vec![
                Complex64::new(-0.2, -0.1),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.1, 0.0),
                Complex64::new(0.8, 0.0),
            ],
        ];
        assert_eq!(dimension, gg.len());

        let rotated = rotgmatrix(0, 1.0, Complex64::new(1.0, 0.0), 0.4, nsp, lx, &gg)
            .expect("rotation should succeed");

        assert_eq!(rotated, gg);
    }

    #[test]
    fn beta_zero_with_unit_phase_preserves_matrix() {
        let nsp = 1;
        let lx = 1;
        let gg = vec![
            vec![
                Complex64::new(1.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(2.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(3.0, 0.0),
                Complex64::new(0.0, 0.0),
            ],
            vec![
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(0.0, 0.0),
                Complex64::new(4.0, 0.0),
            ],
        ];

        let rotated = rotgmatrix(1, 1.0, Complex64::new(1.0, 0.0), 0.0, nsp, lx, &gg)
            .expect("rotation should succeed");

        for row in 0..gg.len() {
            for col in 0..gg[row].len() {
                assert_close(rotated[row][col], gg[row][col], 1.0e-12);
            }
        }
    }
}
