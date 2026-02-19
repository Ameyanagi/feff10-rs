#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum FdmoccError {
    #[error("orbital index out of range: index={index}, available={available}")]
    OrbitalOutOfRange { index: usize, available: usize },
    #[error("invalid kappa for diagonal occupancy at orbital {index}: {kappa}")]
    InvalidKappa { index: usize, kappa: i32 },
}

pub fn fdmocc(i: usize, j: usize, xnel: &[f64], kap: &[i32]) -> Result<f64, FdmoccError> {
    validate_index(i, xnel.len())?;
    validate_index(j, xnel.len())?;
    validate_index(i, kap.len())?;
    validate_index(j, kap.len())?;

    if i == j {
        let kappa = kap[i - 1];
        let a = 2.0 * (kappa.abs() as f64);
        if a <= 1.0 {
            return Err(FdmoccError::InvalidKappa { index: i, kappa });
        }
        let value = xnel[i - 1] * (xnel[j - 1] - 1.0);
        return Ok(value * a / (a - 1.0));
    }

    Ok(xnel[i - 1] * xnel[j - 1])
}

fn validate_index(index: usize, available: usize) -> Result<(), FdmoccError> {
    if index == 0 || index > available {
        return Err(FdmoccError::OrbitalOutOfRange { index, available });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{FdmoccError, fdmocc};

    #[test]
    fn fdmocc_returns_cross_occupancy_product() {
        let value = fdmocc(1, 2, &[2.0, 3.0], &[1, -1]).expect("valid occupancy product");
        assert!((value - 6.0).abs() <= 1.0e-12);
    }

    #[test]
    fn fdmocc_applies_diagonal_relativistic_scaling() {
        let value = fdmocc(1, 1, &[2.0], &[1]).expect("valid diagonal occupancy");
        assert!((value - 4.0).abs() <= 1.0e-12);
    }

    #[test]
    fn fdmocc_rejects_out_of_range_orbital_index() {
        let error = fdmocc(2, 1, &[2.0], &[1]).expect_err("invalid orbital index should fail");
        assert_eq!(
            error,
            FdmoccError::OrbitalOutOfRange {
                index: 2,
                available: 1
            }
        );
    }
}
