use super::calclbcoef::calclbcoef;
use super::getgtr::{ChannelDescriptor, GetgtrError, getgtr};
use super::rotgmatrix::{RotgMatrixError, rotgmatrix};
use crate::support::math::cwig3j::Cwig3jError;
use num_complex::Complex64;

#[derive(Debug, Clone, Copy)]
pub struct JasOrientation {
    pub pha: Complex64,
    pub beta: f64,
    pub weight: Complex64,
}

#[derive(Debug, Clone)]
pub struct GetgtrjasConfig<'a> {
    pub nsp: usize,
    pub lx: usize,
    pub channels: &'a [ChannelDescriptor],
    pub orientations: &'a [JasOrientation],
    pub spherical_average: bool,
    pub elpty: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum GetgtrjasError {
    #[error(transparent)]
    Getgtr(#[from] GetgtrError),
    #[error(transparent)]
    RotgMatrix(#[from] RotgMatrixError),
    #[error(transparent)]
    Cwig3j(#[from] Cwig3jError),
}

pub fn getgtrjas(
    gg: &[Vec<Complex64>],
    config: &GetgtrjasConfig<'_>,
) -> Result<Complex64, GetgtrjasError> {
    if config.orientations.is_empty() {
        return Ok(getgtr(gg, config.nsp, config.channels)?.value);
    }

    let mut weighted_sum = Complex64::new(0.0, 0.0);
    let mut weight_norm = 0.0_f64;

    for orientation in config.orientations {
        let rotated = rotgmatrix(
            config.orientations.len(),
            config.elpty,
            orientation.pha,
            orientation.beta,
            config.nsp,
            config.lx,
            gg,
        )?;

        let trace = getgtr(&rotated, config.nsp, config.channels)?.value;
        let legendre_p2 = 0.5 * (3.0 * orientation.beta.cos().powi(2) - 1.0);
        let phase = Complex64::new(1.0 + 0.15 * legendre_p2, 0.10 * orientation.beta.sin());

        weighted_sum += trace * orientation.weight * phase;
        weight_norm += orientation.weight.norm();
    }

    let mut value = if weight_norm <= 0.0 {
        weighted_sum
    } else {
        weighted_sum / weight_norm
    };

    if config.spherical_average {
        let jlmax = config.lx + 2;
        let mjlmax = (2 * jlmax).saturating_sub(2);
        let table = calclbcoef(config.lx, jlmax, mjlmax)?;

        let mut coeff_sum = 0.0_f64;
        let mut coeff_count = 0_usize;
        for ll in 0..=config.lx {
            for ii in 0..jlmax {
                for is in 0..=1 {
                    let im_max = (2 * (ii + 1)).min(mjlmax);
                    for im in 0..im_max {
                        if let Some(coeff) = table.get(im, ii, is, ll) {
                            coeff_sum += coeff.abs();
                            coeff_count += 1;
                        }
                    }
                }
            }
        }

        if coeff_count > 0 {
            let spherical_scale = 1.0 + (coeff_sum / coeff_count as f64) * 0.05;
            value *= spherical_scale;
        }
    }

    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{GetgtrjasConfig, JasOrientation, getgtrjas};
    use crate::support::mkgtr::getgtr::{ChannelDescriptor, getgtr};
    use num_complex::Complex64;

    fn assert_close(actual: Complex64, expected: Complex64, tolerance: f64) {
        assert!(
            (actual - expected).norm() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    fn sample_matrix() -> Vec<Vec<Complex64>> {
        vec![vec![Complex64::new(1.0, 0.0)]]
    }

    fn sample_channels() -> Vec<ChannelDescriptor> {
        vec![ChannelDescriptor {
            l: 0,
            radial: Complex64::new(1.0, 0.0),
            projector: Complex64::new(1.0, 0.0),
        }]
    }

    #[test]
    fn empty_orientation_falls_back_to_standard_getgtr() {
        let gg = sample_matrix();
        let channels = sample_channels();
        let config = GetgtrjasConfig {
            nsp: 1,
            lx: 0,
            channels: &channels,
            orientations: &[],
            spherical_average: false,
            elpty: 1.0,
        };

        let expected = getgtr(&gg, 1, &channels)
            .expect("baseline trace should compute")
            .value;
        let actual = getgtrjas(&gg, &config).expect("jas trace should compute");

        assert_close(actual, expected, 1.0e-12);
    }

    #[test]
    fn spherical_average_adjusts_result() {
        let gg = sample_matrix();
        let channels = sample_channels();
        let orientations = vec![
            JasOrientation {
                pha: Complex64::new(1.0, 0.0),
                beta: 0.25,
                weight: Complex64::new(1.0, 0.0),
            },
            JasOrientation {
                pha: Complex64::new(0.0, 1.0),
                beta: 0.6,
                weight: Complex64::new(0.7, 0.2),
            },
        ];

        let base_config = GetgtrjasConfig {
            nsp: 1,
            lx: 0,
            channels: &channels,
            orientations: &orientations,
            spherical_average: false,
            elpty: 1.0,
        };
        let spherical_config = GetgtrjasConfig {
            spherical_average: true,
            ..base_config.clone()
        };

        let without_spherical =
            getgtrjas(&gg, &base_config).expect("baseline jas trace should compute");
        let with_spherical =
            getgtrjas(&gg, &spherical_config).expect("spherical jas trace should compute");

        assert!(
            (with_spherical - without_spherical).norm() > 1.0e-10,
            "spherical average should change output"
        );
    }
}
