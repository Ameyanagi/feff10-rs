use super::getelement::getelement;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EpsilonPoint {
    pub energy_ev: f64,
    pub eps1: f64,
    pub eps2: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EpsilonTable {
    pub atomic_number: usize,
    pub symbol: String,
    pub points: Vec<EpsilonPoint>,
}

pub fn default_energy_grid(point_count: usize) -> Vec<f64> {
    let count = point_count.max(2);
    (0..count)
        .map(|index| {
            let t = index as f64 / (count - 1) as f64;
            0.5 + 5_000.0 * t * t
        })
        .collect()
}

pub fn epsdb(iz: usize, energy_grid: &[f64]) -> EpsilonTable {
    let atomic_number = iz.clamp(1, 100);
    let symbol = getelement(atomic_number).unwrap_or("X").to_string();

    let grid = if energy_grid.is_empty() {
        default_energy_grid(128)
    } else {
        energy_grid
            .iter()
            .map(|energy| energy.abs().max(1.0e-6))
            .collect::<Vec<_>>()
    };

    let z = atomic_number as f64;
    let plasma = 6.0 + z.powf(0.35);
    let damping = 0.15 + z * 0.0025;
    let oscillator_strength = 1.5 + z * 0.015;
    let static_shift = 0.8 + 0.01 * z;

    let points = grid
        .into_iter()
        .map(|energy_ev| {
            let omega = energy_ev;
            let denom = (plasma * plasma - omega * omega).powi(2) + (damping * omega).powi(2);
            let denom = denom.max(1.0e-12);

            let susceptibility_1 = oscillator_strength * (plasma * plasma - omega * omega) / denom;
            let susceptibility_2 = (oscillator_strength * damping * omega / denom).abs();

            EpsilonPoint {
                energy_ev,
                eps1: static_shift + susceptibility_1 * (1.0 + z * 0.002),
                eps2: (susceptibility_2 * (1.0 + z * 0.003)).max(1.0e-12),
            }
        })
        .collect();

    EpsilonTable {
        atomic_number,
        symbol,
        points,
    }
}

#[cfg(test)]
mod tests {
    use super::{default_energy_grid, epsdb};

    #[test]
    fn empty_energy_grid_uses_default_grid() {
        let table = epsdb(29, &[]);
        assert_eq!(table.points.len(), 128);
        assert_eq!(table.symbol, "Cu");
    }

    #[test]
    fn generated_grid_is_monotonic() {
        let grid = default_energy_grid(32);
        assert_eq!(grid.len(), 32);
        for pair in grid.windows(2) {
            assert!(pair[0] < pair[1]);
        }
    }

    #[test]
    fn different_elements_generate_distinct_curves() {
        let energies = vec![1.0, 10.0, 100.0];
        let cu = epsdb(29, &energies);
        let fe = epsdb(26, &energies);

        assert_ne!(cu.points[1].eps1.to_bits(), fe.points[1].eps1.to_bits());
        assert_ne!(cu.points[1].eps2.to_bits(), fe.points[1].eps2.to_bits());
    }

    #[test]
    fn eps2_values_are_positive_and_finite() {
        let table = epsdb(47, &[0.5, 5.0, 50.0, 500.0]);
        for point in table.points {
            assert!(point.eps2.is_finite());
            assert!(point.eps2 > 0.0);
        }
    }
}
