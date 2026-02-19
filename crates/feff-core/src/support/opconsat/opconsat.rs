use super::addeps::{AddEpsError, WeightedEpsilon, addeps, loss_from_epsilon, sample_at_energy};
use super::epsdb::{EpsilonPoint, epsdb};

#[derive(Debug, Clone, Copy)]
pub struct OpconsatComponent {
    pub atomic_number: usize,
    pub number_density: f64,
}

#[derive(Debug, Clone)]
pub struct OpconsatResult {
    pub epsilon: Vec<EpsilonPoint>,
    pub loss: Vec<(f64, f64)>,
    pub component_symbols: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum OpconsatError {
    #[error("opconsat requires at least one component")]
    EmptyComponents,
    #[error("all component densities are zero or non-finite")]
    InvalidDensity,
    #[error(transparent)]
    AddEps(#[from] AddEpsError),
}

pub fn opconsat(
    components: &[OpconsatComponent],
    energy_grid: &[f64],
) -> Result<OpconsatResult, OpconsatError> {
    if components.is_empty() {
        return Err(OpconsatError::EmptyComponents);
    }

    let mut positive_density = 0.0_f64;
    for component in components {
        if component.number_density.is_finite() && component.number_density > 0.0 {
            positive_density += component.number_density;
        }
    }
    if positive_density <= 0.0 {
        return Err(OpconsatError::InvalidDensity);
    }

    let mut weighted_components = Vec::with_capacity(components.len());
    let mut component_symbols = Vec::with_capacity(components.len());

    for component in components {
        let normalized_weight =
            if component.number_density.is_finite() && component.number_density > 0.0 {
                component.number_density / positive_density
            } else {
                0.0
            };

        let table = epsdb(component.atomic_number, energy_grid);
        component_symbols.push(table.symbol);
        weighted_components.push(WeightedEpsilon {
            weight: normalized_weight,
            points: table.points,
        });
    }

    let epsilon = addeps(&weighted_components)?;
    let loss = epsilon
        .iter()
        .map(|point| (point.energy_ev, loss_from_epsilon(*point)))
        .collect::<Vec<_>>();

    Ok(OpconsatResult {
        epsilon,
        loss,
        component_symbols,
    })
}

pub fn sample_dielectric(result: &OpconsatResult, energy_ev: f64) -> Option<(f64, f64, f64)> {
    let point = sample_at_energy(&result.epsilon, energy_ev)?;
    Some((point.eps1, point.eps2, loss_from_epsilon(point)))
}

#[cfg(test)]
mod tests {
    use super::{OpconsatComponent, OpconsatError, opconsat, sample_dielectric};

    #[test]
    fn rejects_empty_component_list() {
        let error = opconsat(&[], &[1.0, 2.0, 3.0]).expect_err("empty list should fail");
        assert!(matches!(error, OpconsatError::EmptyComponents));
    }

    #[test]
    fn combines_components_and_generates_loss() {
        let result = opconsat(
            &[
                OpconsatComponent {
                    atomic_number: 29,
                    number_density: 0.8,
                },
                OpconsatComponent {
                    atomic_number: 8,
                    number_density: 0.2,
                },
            ],
            &[1.0, 5.0, 10.0, 20.0],
        )
        .expect("opconsat should compute combined spectrum");

        assert_eq!(result.epsilon.len(), 4);
        assert_eq!(result.loss.len(), 4);
        assert_eq!(result.component_symbols.len(), 2);

        for (_, loss) in result.loss {
            assert!(loss.is_finite());
            assert!(loss >= 0.0);
        }
    }

    #[test]
    fn composition_order_does_not_change_result() {
        let first = opconsat(
            &[
                OpconsatComponent {
                    atomic_number: 29,
                    number_density: 0.6,
                },
                OpconsatComponent {
                    atomic_number: 14,
                    number_density: 0.4,
                },
            ],
            &[1.0, 3.0, 7.0],
        )
        .expect("first composition should compute");

        let second = opconsat(
            &[
                OpconsatComponent {
                    atomic_number: 14,
                    number_density: 0.4,
                },
                OpconsatComponent {
                    atomic_number: 29,
                    number_density: 0.6,
                },
            ],
            &[1.0, 3.0, 7.0],
        )
        .expect("second composition should compute");

        for (lhs, rhs) in first.epsilon.iter().zip(second.epsilon.iter()) {
            assert_eq!(lhs.energy_ev.to_bits(), rhs.energy_ev.to_bits());
            assert_eq!(lhs.eps1.to_bits(), rhs.eps1.to_bits());
            assert_eq!(lhs.eps2.to_bits(), rhs.eps2.to_bits());
        }
    }

    #[test]
    fn dielectric_sampling_interpolates() {
        let result = opconsat(
            &[OpconsatComponent {
                atomic_number: 29,
                number_density: 1.0,
            }],
            &[1.0, 2.0, 4.0],
        )
        .expect("opconsat should compute");

        let sample = sample_dielectric(&result, 1.5).expect("sample should be available");
        assert!(sample.0.is_finite());
        assert!(sample.1.is_finite());
        assert!(sample.2.is_finite());
    }
}
