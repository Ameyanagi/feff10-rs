use super::epsdb::EpsilonPoint;

#[derive(Debug, Clone)]
pub struct WeightedEpsilon {
    pub weight: f64,
    pub points: Vec<EpsilonPoint>,
}

#[derive(Debug, thiserror::Error)]
pub enum AddEpsError {
    #[error("addeps requires at least one epsilon component")]
    EmptyInput,
    #[error("component #{index} has no valid epsilon points")]
    EmptyComponent { index: usize },
}

pub fn addeps(inputs: &[WeightedEpsilon]) -> Result<Vec<EpsilonPoint>, AddEpsError> {
    if inputs.is_empty() {
        return Err(AddEpsError::EmptyInput);
    }

    let mut prepared = Vec::with_capacity(inputs.len());
    let mut union_grid = Vec::<f64>::new();

    for (index, input) in inputs.iter().enumerate() {
        let mut points = input
            .points
            .iter()
            .copied()
            .filter(|point| {
                point.energy_ev.is_finite() && point.eps1.is_finite() && point.eps2.is_finite()
            })
            .collect::<Vec<_>>();

        if points.is_empty() {
            return Err(AddEpsError::EmptyComponent { index });
        }

        points.sort_by(|lhs, rhs| lhs.energy_ev.total_cmp(&rhs.energy_ev));
        for point in &points {
            union_grid.push(point.energy_ev);
        }

        prepared.push(WeightedEpsilon {
            weight: input.weight,
            points,
        });
    }

    union_grid.sort_by(f64::total_cmp);
    union_grid.dedup_by(|lhs, rhs| (*lhs - *rhs).abs() <= 1.0e-9);

    let mut combined = Vec::with_capacity(union_grid.len());
    for &energy in &union_grid {
        let mut eps1_sum = 0.0_f64;
        let mut eps2_sum = 0.0_f64;

        for component in &prepared {
            let sample = interpolate(&component.points, energy);
            eps1_sum += component.weight * sample.eps1;
            eps2_sum += component.weight * sample.eps2;
        }

        combined.push(EpsilonPoint {
            energy_ev: energy,
            eps1: eps1_sum + 1.0,
            eps2: eps2_sum,
        });
    }

    Ok(combined)
}

pub fn loss_from_epsilon(point: EpsilonPoint) -> f64 {
    let denom = (point.eps1 * point.eps1 + point.eps2 * point.eps2).max(1.0e-18);
    point.eps2 / denom
}

pub fn sample_at_energy(points: &[EpsilonPoint], energy_ev: f64) -> Option<EpsilonPoint> {
    if points.is_empty() {
        return None;
    }

    Some(interpolate(points, energy_ev.abs().max(1.0e-6)))
}

fn interpolate(points: &[EpsilonPoint], energy_ev: f64) -> EpsilonPoint {
    if points.len() == 1 {
        return points[0];
    }

    if energy_ev <= points[0].energy_ev {
        return points[0];
    }
    if energy_ev >= points[points.len() - 1].energy_ev {
        return points[points.len() - 1];
    }

    let upper = points
        .partition_point(|point| point.energy_ev < energy_ev)
        .min(points.len() - 1);
    let lower = upper.saturating_sub(1);

    let lhs = points[lower];
    let rhs = points[upper];
    if (rhs.energy_ev - lhs.energy_ev).abs() <= f64::EPSILON {
        return lhs;
    }

    let t = (energy_ev - lhs.energy_ev) / (rhs.energy_ev - lhs.energy_ev);
    EpsilonPoint {
        energy_ev,
        eps1: lhs.eps1 + (rhs.eps1 - lhs.eps1) * t,
        eps2: lhs.eps2 + (rhs.eps2 - lhs.eps2) * t,
    }
}

#[cfg(test)]
mod tests {
    use super::{WeightedEpsilon, addeps, loss_from_epsilon, sample_at_energy};
    use crate::support::opconsat::epsdb::EpsilonPoint;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn addeps_merges_union_grid_with_linear_interpolation() {
        let inputs = vec![
            WeightedEpsilon {
                weight: 0.5,
                points: vec![
                    EpsilonPoint {
                        energy_ev: 1.0,
                        eps1: 1.0,
                        eps2: 2.0,
                    },
                    EpsilonPoint {
                        energy_ev: 3.0,
                        eps1: 3.0,
                        eps2: 4.0,
                    },
                ],
            },
            WeightedEpsilon {
                weight: 1.5,
                points: vec![
                    EpsilonPoint {
                        energy_ev: 2.0,
                        eps1: 2.0,
                        eps2: 1.0,
                    },
                    EpsilonPoint {
                        energy_ev: 3.0,
                        eps1: 4.0,
                        eps2: 2.0,
                    },
                ],
            },
        ];

        let merged = addeps(&inputs).expect("addeps should merge components");
        assert_eq!(merged.len(), 3);
        assert_close(merged[0].energy_ev, 1.0, 1.0e-12);
        assert_close(merged[1].energy_ev, 2.0, 1.0e-12);
        assert_close(merged[2].energy_ev, 3.0, 1.0e-12);

        // At 2.0 eV, component-1 interpolates eps1=2, eps2=3. Then +1 dielectric offset.
        assert_close(merged[1].eps1, (0.5 * 2.0 + 1.5 * 2.0) + 1.0, 1.0e-12);
        assert_close(merged[1].eps2, 0.5 * 3.0 + 1.5 * 1.0, 1.0e-12);
    }

    #[test]
    fn loss_function_is_positive_for_positive_imaginary_part() {
        let point = EpsilonPoint {
            energy_ev: 10.0,
            eps1: 2.0,
            eps2: 0.5,
        };
        let loss = loss_from_epsilon(point);
        assert!(loss.is_finite());
        assert!(loss > 0.0);
    }

    #[test]
    fn sample_at_energy_interpolates_between_points() {
        let points = vec![
            EpsilonPoint {
                energy_ev: 1.0,
                eps1: 1.0,
                eps2: 2.0,
            },
            EpsilonPoint {
                energy_ev: 3.0,
                eps1: 5.0,
                eps2: 6.0,
            },
        ];

        let sampled = sample_at_energy(&points, 2.0).expect("sample should exist");
        assert_close(sampled.eps1, 3.0, 1.0e-12);
        assert_close(sampled.eps2, 4.0, 1.0e-12);
    }
}
