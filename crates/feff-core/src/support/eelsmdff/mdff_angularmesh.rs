use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AngularMeshConfig {
    pub theta_x_center: f64,
    pub theta_y_center: f64,
    pub npos: usize,
    pub nqr: usize,
    pub nqf: usize,
    pub qmodus: char,
    pub th0: f64,
    pub thpart: f64,
    pub acoll: f64,
    pub aconv: f64,
    pub legacy_manual_hack: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AngularMesh {
    pub theta_x: Vec<f64>,
    pub theta_y: Vec<f64>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum AngularMeshError {
    #[error("npos must be at least 1")]
    EmptyMesh,
    #[error("nqr must be at least 1")]
    InvalidRadialCount,
    #[error("nqf must be at least 1")]
    InvalidAngularCount,
    #[error("unsupported qmodus '{mode}'; expected 'U', 'L', or '1'")]
    UnsupportedMode { mode: char },
    #[error("legacy manual hack requires npos >= 2")]
    LegacyHackRequiresTwoPoints,
    #[error("logarithmic/1d mode requires th0 > 0")]
    InvalidTh0,
    #[error("logarithmic mode requires aconv + acoll > 0 when nqr > 1")]
    InvalidLogRadius,
    #[error("configured npos={configured} does not match generated point count={generated}")]
    NposMismatch { configured: usize, generated: usize },
}

pub fn mdff_angularmesh(config: &AngularMeshConfig) -> Result<AngularMesh, AngularMeshError> {
    if config.npos == 0 {
        return Err(AngularMeshError::EmptyMesh);
    }

    if config.legacy_manual_hack {
        if config.npos < 2 {
            return Err(AngularMeshError::LegacyHackRequiresTwoPoints);
        }

        let mut theta_x = vec![0.0; config.npos];
        let mut theta_y = vec![0.0; config.npos];
        theta_x[0] = 0.0;
        theta_y[0] = 0.0;
        theta_x[1] = 0.0;
        theta_y[1] = 0.002;

        return Ok(AngularMesh { theta_x, theta_y });
    }

    if config.nqr == 0 {
        return Err(AngularMeshError::InvalidRadialCount);
    }
    if config.nqf == 0 {
        return Err(AngularMeshError::InvalidAngularCount);
    }

    let is_log_mode = match config.qmodus {
        'U' => false,
        'L' | '1' => true,
        other => return Err(AngularMeshError::UnsupportedMode { mode: other }),
    };

    if config.npos == 1 {
        return Ok(AngularMesh {
            theta_x: vec![config.theta_x_center],
            theta_y: vec![config.theta_y_center],
        });
    }

    if is_log_mode && config.th0 <= 0.0 {
        return Err(AngularMeshError::InvalidTh0);
    }

    let dxx = if is_log_mode && config.nqr > 1 {
        let radius = config.acoll + config.aconv;
        if radius <= 0.0 {
            return Err(AngularMeshError::InvalidLogRadius);
        }
        (radius / config.th0).ln() / (config.nqr.saturating_sub(1) as f64)
    } else {
        0.0
    };

    let mut theta_x = Vec::with_capacity(config.npos);
    let mut theta_y = Vec::with_capacity(config.npos);

    for ir in 1..=config.nqr {
        let mut n_present_tour = config.nqf * (2 * ir - 1);
        if config.qmodus == '1' {
            n_present_tour = 1;
        }
        let inter_angle = 2.0 * PI / n_present_tour as f64;

        for itour in 1..=n_present_tour {
            let angle = inter_angle * itour as f64;

            let (x, y) = if is_log_mode {
                if ir == 1 {
                    let radius = config.th0 / 2.0;
                    (
                        config.theta_x_center + radius * angle.cos(),
                        config.theta_y_center + radius * angle.sin(),
                    )
                } else {
                    let radial_scale =
                        config.th0 * (dxx * (ir as f64 - 2.0)).exp() * (1.0 + dxx.exp()) / 2.0;
                    (
                        config.theta_x_center + radial_scale * angle.cos(),
                        config.theta_y_center + radial_scale * angle.sin(),
                    )
                }
            } else {
                let radius = config.thpart * (2 * ir - 1) as f64;
                (
                    config.theta_x_center + radius * angle.cos(),
                    config.theta_y_center + radius * angle.sin(),
                )
            };

            theta_x.push(x);
            theta_y.push(y);
        }
    }

    if theta_x.len() != config.npos {
        return Err(AngularMeshError::NposMismatch {
            configured: config.npos,
            generated: theta_x.len(),
        });
    }

    Ok(AngularMesh { theta_x, theta_y })
}

#[cfg(test)]
mod tests {
    use super::{AngularMeshConfig, AngularMeshError, mdff_angularmesh};

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn legacy_hack_returns_two_manual_points() {
        let mesh = mdff_angularmesh(&AngularMeshConfig {
            theta_x_center: 0.2,
            theta_y_center: -0.3,
            npos: 2,
            nqr: 1,
            nqf: 1,
            qmodus: 'L',
            th0: 0.001,
            thpart: 0.0,
            acoll: 0.0,
            aconv: 0.0,
            legacy_manual_hack: true,
        })
        .expect("legacy mesh should be generated");

        assert_eq!(mesh.theta_x, vec![0.0, 0.0]);
        assert_eq!(mesh.theta_y, vec![0.0, 0.002]);
    }

    #[test]
    fn single_position_returns_center_point() {
        let mesh = mdff_angularmesh(&AngularMeshConfig {
            theta_x_center: 0.02,
            theta_y_center: -0.03,
            npos: 1,
            nqr: 1,
            nqf: 1,
            qmodus: 'U',
            th0: 0.001,
            thpart: 0.0005,
            acoll: 0.0,
            aconv: 0.0,
            legacy_manual_hack: false,
        })
        .expect("single point should be accepted");

        assert_eq!(mesh.theta_x, vec![0.02]);
        assert_eq!(mesh.theta_y, vec![-0.03]);
    }

    #[test]
    fn uniform_mode_uses_linear_ring_spacing() {
        let mesh = mdff_angularmesh(&AngularMeshConfig {
            theta_x_center: 0.0,
            theta_y_center: 0.0,
            npos: 4,
            nqr: 2,
            nqf: 1,
            qmodus: 'U',
            th0: 0.001,
            thpart: 0.001,
            acoll: 0.0,
            aconv: 0.0,
            legacy_manual_hack: false,
        })
        .expect("uniform mesh should be generated");

        let radii = mesh
            .theta_x
            .iter()
            .zip(mesh.theta_y.iter())
            .map(|(x, y)| (x * x + y * y).sqrt())
            .collect::<Vec<_>>();

        assert_eq!(radii.len(), 4);
        assert_close(radii[0], 0.001, 1.0e-12);
        assert_close(radii[1], 0.003, 1.0e-12);
        assert_close(radii[2], 0.003, 1.0e-12);
        assert_close(radii[3], 0.003, 1.0e-12);
    }

    #[test]
    fn logarithmic_mode_uses_th0_seed_and_log_scaling() {
        let mesh = mdff_angularmesh(&AngularMeshConfig {
            theta_x_center: 0.0,
            theta_y_center: 0.0,
            npos: 4,
            nqr: 2,
            nqf: 1,
            qmodus: 'L',
            th0: 0.001,
            thpart: 0.0,
            acoll: 0.003,
            aconv: 0.001,
            legacy_manual_hack: false,
        })
        .expect("log mesh should be generated");

        let radii = mesh
            .theta_x
            .iter()
            .zip(mesh.theta_y.iter())
            .map(|(x, y)| (x * x + y * y).sqrt())
            .collect::<Vec<_>>();

        assert_close(radii[0], 0.0005, 1.0e-12);
        assert_close(radii[1], 0.0025, 1.0e-12);
        assert_close(radii[2], 0.0025, 1.0e-12);
        assert_close(radii[3], 0.0025, 1.0e-12);
    }

    #[test]
    fn reports_mismatch_when_npos_does_not_match_generation_rule() {
        let error = mdff_angularmesh(&AngularMeshConfig {
            theta_x_center: 0.0,
            theta_y_center: 0.0,
            npos: 5,
            nqr: 2,
            nqf: 1,
            qmodus: 'U',
            th0: 0.001,
            thpart: 0.001,
            acoll: 0.0,
            aconv: 0.0,
            legacy_manual_hack: false,
        })
        .expect_err("npos mismatch should fail");

        assert_eq!(
            error,
            AngularMeshError::NposMismatch {
                configured: 5,
                generated: 4
            }
        );
    }
}
