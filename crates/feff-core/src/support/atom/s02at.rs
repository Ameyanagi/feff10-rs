use crate::support::math::determ::determ;

const KAP_MIN: i32 = -5;
const KAP_MAX: i32 = 4;
const MAX_KAP_ORBITALS: usize = 8;

#[derive(Debug, Clone)]
pub struct S02atInput<'a> {
    pub ihole: usize,
    pub norb: usize,
    pub nk: &'a [i32],
    pub xnel: &'a [f64],
    pub ovpint: &'a [Vec<f64>],
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum S02atError {
    #[error("norb must be >= 1")]
    InvalidNorb,
    #[error("input length mismatch for {name}: need at least {need}, got {got}")]
    LengthMismatch {
        name: &'static str,
        need: usize,
        got: usize,
    },
    #[error("overlap matrix row {row} has length {got}, need at least {need}")]
    OverlapRowTooShort { row: usize, got: usize, need: usize },
    #[error("more than {max} orbitals share kappa={kappa}")]
    TooManyOrbitalsForKappa { max: usize, kappa: i32 },
    #[error("determinant failed for matrix size {0}")]
    DeterminantFailed(usize),
}

pub fn s02at(input: &S02atInput<'_>) -> Result<f64, S02atError> {
    if input.norb == 0 {
        return Err(S02atError::InvalidNorb);
    }
    ensure_len("nk", input.nk.len(), input.norb)?;
    ensure_len("xnel", input.xnel.len(), input.norb)?;
    ensure_len("ovpint", input.ovpint.len(), input.norb)?;

    let mut row = 0usize;
    while row < input.norb {
        if input.ovpint[row].len() < input.norb {
            return Err(S02atError::OverlapRowTooShort {
                row: row + 1,
                got: input.ovpint[row].len(),
                need: input.norb,
            });
        }
        row += 1;
    }

    let mut dval = 1.0_f64;

    let mut kap = KAP_MIN;
    while kap <= KAP_MAX {
        let mut m1 = identity_matrix();
        let mut morb = 0usize;
        let mut nhole = 0usize;
        let mut iorb = [0usize; MAX_KAP_ORBITALS];

        let mut i = 0usize;
        while i < input.norb {
            if input.nk[i] == kap {
                if morb == MAX_KAP_ORBITALS {
                    return Err(S02atError::TooManyOrbitalsForKappa {
                        max: MAX_KAP_ORBITALS,
                        kappa: kap,
                    });
                }

                iorb[morb] = i;
                morb += 1;

                let mut j = 0usize;
                while j < morb {
                    m1[j][morb - 1] = input.ovpint[iorb[j]][iorb[morb - 1]];
                    j += 1;
                }

                let mut j2 = 0usize;
                while j2 + 1 < morb {
                    m1[morb - 1][j2] = m1[j2][morb - 1];
                    j2 += 1;
                }

                if input.ihole == i + 1 {
                    nhole = morb;
                }
            }
            i += 1;
        }

        if morb == 0 {
            kap += 1;
            continue;
        }

        let dum1 = determinant_squared(&m1, morb)?;
        let dum3 = determinant_squared(&m1, morb.saturating_sub(1))?;
        let xn = input.xnel[iorb[morb - 1]];
        let nmax = 2.0 * kap.abs() as f64;
        let xnh = nmax - xn;

        if nhole == 0 {
            dval *= dum1.powf(xn) * dum3.powf(xnh);
        } else if nhole == morb {
            dval *= dum1.powf(xn - 1.0) * dum3.powf(xnh + 1.0);
        } else {
            let mut m2 = m1;
            elimin(&m1, nhole, &mut m2);

            let dum2 = determinant_squared(&m2, morb)?;
            let dum4 = determinant_squared(&m2, morb.saturating_sub(1))?;
            let dum5 = (dum4 * dum1 * xnh + dum2 * dum3 * xn) / nmax;
            dval *= dum5 * dum1.powf(xn - 1.0) * dum3.powf(xnh - 1.0);
        }

        kap += 1;
    }

    Ok(dval)
}

fn identity_matrix() -> [[f64; MAX_KAP_ORBITALS]; MAX_KAP_ORBITALS] {
    let mut matrix = [[0.0_f64; MAX_KAP_ORBITALS]; MAX_KAP_ORBITALS];
    let mut i = 0usize;
    while i < MAX_KAP_ORBITALS {
        matrix[i][i] = 1.0;
        i += 1;
    }
    matrix
}

fn determinant_squared(
    matrix: &[[f64; MAX_KAP_ORBITALS]; MAX_KAP_ORBITALS],
    order: usize,
) -> Result<f64, S02atError> {
    let mut work = vec![vec![0.0_f64; order]; order];

    let mut i = 0usize;
    while i < order {
        let mut j = 0usize;
        while j < order {
            work[i][j] = matrix[i][j];
            j += 1;
        }
        i += 1;
    }

    let value = determ(&mut work, order).ok_or(S02atError::DeterminantFailed(order))?;
    Ok(value * value)
}

fn elimin(
    source: &[[f64; MAX_KAP_ORBITALS]; MAX_KAP_ORBITALS],
    n: usize,
    target: &mut [[f64; MAX_KAP_ORBITALS]; MAX_KAP_ORBITALS],
) {
    let mut i = 0usize;
    while i < MAX_KAP_ORBITALS {
        let mut j = 0usize;
        while j < MAX_KAP_ORBITALS {
            target[i][j] = if i + 1 != n {
                if j + 1 != n { source[i][j] } else { 0.0 }
            } else if j + 1 != n {
                0.0
            } else {
                1.0
            };
            j += 1;
        }
        i += 1;
    }
}

fn ensure_len(name: &'static str, got: usize, need: usize) -> Result<(), S02atError> {
    if got < need {
        return Err(S02atError::LengthMismatch { name, need, got });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{S02atError, S02atInput, s02at};

    #[test]
    fn s02at_returns_unity_for_identity_overlap() {
        let ovpint = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        let value = s02at(&S02atInput {
            ihole: 0,
            norb: 3,
            nk: &[1, 1, -1],
            xnel: &[2.0, 1.0, 2.0],
            ovpint: &ovpint,
        })
        .expect("identity matrix should succeed");

        assert!((value - 1.0).abs() <= 1.0e-12);
    }

    #[test]
    fn s02at_handles_hole_inside_kappa_block() {
        let ovpint = vec![vec![1.0, 0.2], vec![0.2, 1.0]];

        let value = s02at(&S02atInput {
            ihole: 1,
            norb: 2,
            nk: &[1, 1],
            xnel: &[1.5, 0.5],
            ovpint: &ovpint,
        })
        .expect("valid overlap matrix should succeed");

        assert!(value.is_finite());
        assert!(value > 0.0);
    }

    #[test]
    fn s02at_rejects_more_than_eight_orbitals_per_kappa() {
        let mut ovpint = vec![vec![0.0_f64; 9]; 9];
        let mut i = 0usize;
        while i < 9 {
            ovpint[i][i] = 1.0;
            i += 1;
        }

        let error = s02at(&S02atInput {
            ihole: 0,
            norb: 9,
            nk: &[1, 1, 1, 1, 1, 1, 1, 1, 1],
            xnel: &[1.0; 9],
            ovpint: &ovpint,
        })
        .expect_err("kappa block larger than 8 should fail");

        assert_eq!(
            error,
            S02atError::TooManyOrbitalsForKappa { max: 8, kappa: 1 }
        );
    }
}
