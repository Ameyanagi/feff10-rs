use super::mdff_euler::mdff_euler;
use super::mdff_productmatvect::mdff_productmatvect;
use super::mdff_wavelength::mdff_wavelength_with_constants;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MdffQMeshPoint {
    pub theta_x: f64,
    pub theta_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MdffQMeshConfig {
    pub beam_energy_ev: f64,
    pub scattered_energy_ev: f64,
    pub beam_direction: [f64; 3],
    pub relativistic_q: bool,
    pub h_on_sqrt_two_me: f64,
    pub me_c2_ev: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MdffQMeshRow {
    pub q_vector: [f64; 3],
    pub q_length: f64,
    pub q_length_classical: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MdffQMeshResult {
    pub euler_rotation: [[f64; 3]; 3],
    pub rows: Vec<MdffQMeshRow>,
}

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq)]
pub enum MdffQMeshError {
    #[error("q-mesh requires at least one detector point")]
    EmptyDetectorPoints,
    #[error("beam direction must contain at least one non-zero finite component")]
    InvalidBeamDirection,
    #[error("beam energy must be finite and positive, got {value}")]
    InvalidBeamEnergy { value: f64 },
    #[error("scattered energy must be finite and positive, got {value}")]
    InvalidScatteredEnergy { value: f64 },
    #[error("me_c2_ev must be finite and positive, got {value}")]
    InvalidMeC2 { value: f64 },
    #[error("detector point at index {index} contains non-finite angles")]
    NonFiniteDetectorPoint { index: usize },
    #[error("failed to evaluate beam wavelength: {0}")]
    BeamWavelength(#[from] super::mdff_wavelength::MdffWavelengthError),
}

pub fn mdff_qmesh(
    detector_points: &[MdffQMeshPoint],
    config: MdffQMeshConfig,
) -> Result<MdffQMeshResult, MdffQMeshError> {
    if detector_points.is_empty() {
        return Err(MdffQMeshError::EmptyDetectorPoints);
    }
    if !config.beam_energy_ev.is_finite() || config.beam_energy_ev <= 0.0 {
        return Err(MdffQMeshError::InvalidBeamEnergy {
            value: config.beam_energy_ev,
        });
    }
    if !config.scattered_energy_ev.is_finite() || config.scattered_energy_ev <= 0.0 {
        return Err(MdffQMeshError::InvalidScatteredEnergy {
            value: config.scattered_energy_ev,
        });
    }
    if !config.me_c2_ev.is_finite() || config.me_c2_ev <= 0.0 {
        return Err(MdffQMeshError::InvalidMeC2 {
            value: config.me_c2_ev,
        });
    }

    let direction = config.beam_direction;
    if direction.iter().any(|component| !component.is_finite())
        || direction.iter().all(|component| component.abs() <= 1.0e-12)
    {
        return Err(MdffQMeshError::InvalidBeamDirection);
    }

    let euler_rotation = euler_rotation_for_beam(direction);
    let k0_len = 2.0 * PI
        / mdff_wavelength_with_constants(
            config.beam_energy_ev,
            config.h_on_sqrt_two_me,
            config.me_c2_ev,
        )?;
    let kpr_len = 2.0 * PI
        / mdff_wavelength_with_constants(
            config.scattered_energy_ev,
            config.h_on_sqrt_two_me,
            config.me_c2_ev,
        )?;

    let beta = ((2.0 + config.beam_energy_ev / config.me_c2_ev)
        / (2.0
            + config.beam_energy_ev / config.me_c2_ev
            + config.me_c2_ev / config.beam_energy_ev))
        .sqrt();

    let mut rows = Vec::with_capacity(detector_points.len());

    for (index, point) in detector_points.iter().enumerate() {
        if !point.theta_x.is_finite() || !point.theta_y.is_finite() {
            return Err(MdffQMeshError::NonFiniteDetectorPoint { index });
        }

        let theta = (point.theta_x * point.theta_x + point.theta_y * point.theta_y).sqrt();
        let phi = azimuth_from_components(point.theta_x, point.theta_y);

        let mut q_vector = [
            -kpr_len * theta.sin() * phi.cos(),
            -kpr_len * theta.sin() * phi.sin(),
            kpr_len * theta.cos() - k0_len,
        ];
        let q_length_classical = norm3(q_vector);

        if config.relativistic_q {
            q_vector[2] *= 1.0 - beta * beta;
        }

        let q_length = norm3(q_vector);
        let rotated = mdff_productmatvect(euler_rotation, q_vector);

        rows.push(MdffQMeshRow {
            q_vector: rotated,
            q_length,
            q_length_classical,
        });
    }

    Ok(MdffQMeshResult {
        euler_rotation,
        rows,
    })
}

fn euler_rotation_for_beam(beam_direction: [f64; 3]) -> [[f64; 3]; 3] {
    let alpha1 = if beam_direction[0].abs() < 1.0e-4 {
        PI / 2.0
    } else {
        (beam_direction[1] / beam_direction[0]).atan()
    };

    let alpha2 = if beam_direction[2].abs() < 1.0e-4 {
        PI / 2.0
    } else {
        ((beam_direction[0] * beam_direction[0] + beam_direction[1] * beam_direction[1]).sqrt()
            / beam_direction[2])
            .atan()
    };

    mdff_euler(alpha1, alpha2, 0.0)
}

fn azimuth_from_components(theta_x: f64, theta_y: f64) -> f64 {
    if theta_x.abs() < 1.0e-6 {
        if theta_y > 0.0 { PI / 2.0 } else { -PI / 2.0 }
    } else {
        let mut phi = (theta_y / theta_x).atan().abs();
        if theta_y < 0.0 && theta_x < 0.0 {
            phi += PI;
        } else if theta_x < 0.0 {
            phi = PI - phi;
        } else if theta_y < 0.0 {
            phi = -phi;
        }
        phi
    }
}

fn norm3(values: [f64; 3]) -> f64 {
    (values[0] * values[0] + values[1] * values[1] + values[2] * values[2]).sqrt()
}

#[cfg(test)]
mod tests {
    use super::{MdffQMeshConfig, MdffQMeshError, MdffQMeshPoint, mdff_qmesh, norm3};
    use crate::support::eelsmdff::mdff_wavelength::{
        DEFAULT_H_ON_SQRT_TWO_ME_AU, DEFAULT_ME_C2_EV,
    };

    fn baseline_config(relativistic_q: bool) -> MdffQMeshConfig {
        MdffQMeshConfig {
            beam_energy_ev: 200_000.0,
            scattered_energy_ev: 199_980.0,
            beam_direction: [0.0, 0.0, 1.0],
            relativistic_q,
            h_on_sqrt_two_me: DEFAULT_H_ON_SQRT_TWO_ME_AU,
            me_c2_ev: DEFAULT_ME_C2_EV,
        }
    }

    #[test]
    fn qmesh_returns_one_row_per_detector_point() {
        let points = vec![
            MdffQMeshPoint {
                theta_x: 0.001,
                theta_y: 0.0,
            },
            MdffQMeshPoint {
                theta_x: 0.0,
                theta_y: 0.0015,
            },
        ];

        let mesh = mdff_qmesh(&points, baseline_config(false)).expect("qmesh should build");

        assert_eq!(mesh.rows.len(), points.len());
        for row in mesh.rows {
            assert!(row.q_vector.iter().all(|value| value.is_finite()));
            assert!(row.q_length.is_finite());
            assert!(row.q_length_classical.is_finite());
            assert!(row.q_length > 0.0);
            assert!(row.q_length_classical > 0.0);
        }
    }

    #[test]
    fn relativistic_toggle_changes_q_length_but_not_classical_length() {
        let points = vec![MdffQMeshPoint {
            theta_x: 0.0012,
            theta_y: -0.0004,
        }];

        let non_rel = mdff_qmesh(&points, baseline_config(false)).expect("non-rel mesh");
        let rel = mdff_qmesh(&points, baseline_config(true)).expect("rel mesh");

        assert!(
            (non_rel.rows[0].q_length_classical - rel.rows[0].q_length_classical).abs() <= 1.0e-12
        );
        assert!((non_rel.rows[0].q_length - rel.rows[0].q_length).abs() > 1.0e-9);
        assert!((norm3(non_rel.rows[0].q_vector) - non_rel.rows[0].q_length).abs() <= 1.0e-12);
        assert!((norm3(rel.rows[0].q_vector) - rel.rows[0].q_length).abs() <= 1.0e-12);
    }

    #[test]
    fn rejects_invalid_beam_energy() {
        let error = mdff_qmesh(
            &[MdffQMeshPoint {
                theta_x: 0.0,
                theta_y: 0.0,
            }],
            MdffQMeshConfig {
                beam_energy_ev: 0.0,
                ..baseline_config(false)
            },
        )
        .expect_err("invalid beam energy should fail");

        assert_eq!(error, MdffQMeshError::InvalidBeamEnergy { value: 0.0 });
    }

    #[test]
    fn rejects_non_finite_detector_point() {
        let error = mdff_qmesh(
            &[MdffQMeshPoint {
                theta_x: f64::NAN,
                theta_y: 0.0,
            }],
            baseline_config(false),
        )
        .expect_err("non-finite points should fail");

        assert_eq!(error, MdffQMeshError::NonFiniteDetectorPoint { index: 0 });
    }
}
