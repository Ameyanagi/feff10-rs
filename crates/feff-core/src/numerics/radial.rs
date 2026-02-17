use num_complex::Complex64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialExtent {
    pub mean: f64,
    pub rms: f64,
    pub max: f64,
}

impl RadialExtent {
    pub fn new(mean: f64, rms: f64, max: f64) -> Self {
        let max = sanitize_positive(max).max(1.0e-6);
        let mean = sanitize_positive(mean).min(max);
        let rms = sanitize_positive(rms).max(mean).max(1.0e-6);
        Self { mean, rms, max }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadialGrid {
    points: Vec<f64>,
    extent: RadialExtent,
    log_step: f64,
}

impl RadialGrid {
    pub fn from_sampled_radii(
        sampled_radii: &[f64],
        point_count: usize,
        log_step_hint: f64,
    ) -> Self {
        let mut sample_count = 0_usize;
        let mut radius_sum = 0.0_f64;
        let mut radius_sq_sum = 0.0_f64;
        let mut radius_max = 0.0_f64;

        for radius in sampled_radii {
            if !radius.is_finite() || *radius < 0.0 {
                continue;
            }

            radius_sum += *radius;
            radius_sq_sum += radius * radius;
            radius_max = radius_max.max(*radius);
            sample_count += 1;
        }

        let extent = if sample_count == 0 {
            RadialExtent::new(1.0, 1.0, 1.0)
        } else {
            let count = sample_count as f64;
            RadialExtent::new(
                radius_sum / count,
                (radius_sq_sum / count).sqrt(),
                radius_max,
            )
        };

        Self::from_extent(extent, point_count, log_step_hint)
    }

    pub fn from_extent(extent: RadialExtent, point_count: usize, log_step_hint: f64) -> Self {
        let point_count = point_count.max(2);
        let radius_min = (extent.mean.max(1.0e-4) * 1.0e-3).max(1.0e-6);
        let radius_max = extent.max.max(radius_min * 1.01);
        let computed_log_step = (radius_max / radius_min).ln() / (point_count - 1) as f64;

        let log_step = if log_step_hint.is_finite() && log_step_hint > 0.0 {
            log_step_hint.max(1.0e-6)
        } else {
            computed_log_step.max(1.0e-6)
        };

        let mut points = Vec::with_capacity(point_count);
        for index in 0..point_count {
            let exponent = (index as f64 * log_step).clamp(-700.0, 700.0);
            let mut radius = radius_min * exponent.exp();
            if !radius.is_finite() || radius <= 0.0 {
                radius = points
                    .last()
                    .copied()
                    .unwrap_or(radius_min)
                    .mul_add(1.25, 0.0);
            }
            points.push(radius);
        }

        if let Some(last) = points.last().copied() {
            if last.is_finite() && last > 0.0 {
                let scale = radius_max / last;
                for point in &mut points {
                    *point *= scale;
                }
            }
        }

        if let Some(first) = points.first_mut() {
            *first = radius_min;
        }
        if let Some(last) = points.last_mut() {
            *last = radius_max;
        }

        for index in 1..points.len() {
            if points[index] <= points[index - 1] {
                points[index] = points[index - 1] * 1.000_001;
            }
        }

        Self {
            points,
            extent,
            log_step: log_step.max(1.0e-6),
        }
    }

    pub fn points(&self) -> &[f64] {
        &self.points
    }

    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    pub fn log_step(&self) -> f64 {
        self.log_step
    }

    pub fn extent(&self) -> RadialExtent {
        self.extent
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoundStateSolverState {
    radial_grid: RadialGrid,
    iteration_limit: usize,
    mixing_parameter: f64,
    muffin_tin_radius: f64,
}

impl BoundStateSolverState {
    pub fn new(
        radial_grid: RadialGrid,
        iteration_limit: usize,
        mixing_parameter: f64,
        muffin_tin_radius: f64,
    ) -> Self {
        let default_radius = radial_grid.extent().max.max(1.0e-6);
        let muffin_tin_radius = if muffin_tin_radius.is_finite() && muffin_tin_radius > 0.0 {
            muffin_tin_radius
        } else {
            default_radius
        };

        Self {
            radial_grid,
            iteration_limit: iteration_limit.max(1),
            mixing_parameter: mixing_parameter.abs().clamp(0.0, 1.0),
            muffin_tin_radius,
        }
    }

    pub fn radial_grid(&self) -> &RadialGrid {
        &self.radial_grid
    }

    pub fn iteration_limit(&self) -> usize {
        self.iteration_limit
    }

    pub fn mixing_parameter(&self) -> f64 {
        self.mixing_parameter
    }

    pub fn muffin_tin_radius(&self) -> f64 {
        self.muffin_tin_radius
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComplexEnergySolverState {
    radial_grid: RadialGrid,
    energy: Complex64,
    max_wave_number: f64,
    channel_count_hint: usize,
}

impl ComplexEnergySolverState {
    pub fn new(
        radial_grid: RadialGrid,
        energy: Complex64,
        max_wave_number: f64,
        channel_count_hint: usize,
    ) -> Self {
        let energy = if energy.re.is_finite() && energy.im.is_finite() {
            energy
        } else {
            Complex64::new(0.0, 0.0)
        };

        let max_wave_number = if max_wave_number.is_finite() && max_wave_number > 0.0 {
            max_wave_number
        } else {
            energy.im.abs().max(1.0e-4)
        };

        Self {
            radial_grid,
            energy,
            max_wave_number,
            channel_count_hint: channel_count_hint.max(1),
        }
    }

    pub fn radial_grid(&self) -> &RadialGrid {
        &self.radial_grid
    }

    pub fn energy(&self) -> Complex64 {
        self.energy
    }

    pub fn max_wave_number(&self) -> f64 {
        self.max_wave_number
    }

    pub fn channel_count_hint(&self) -> usize {
        self.channel_count_hint
    }
}

fn sanitize_positive(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundStateSolverState, ComplexEnergySolverState, RadialExtent, RadialGrid};
    use num_complex::Complex64;

    #[test]
    fn radial_grid_from_sampled_radii_tracks_extent_and_points() {
        let grid = RadialGrid::from_sampled_radii(&[0.4, 0.8, 1.2], 64, 0.05);
        let extent = grid.extent();

        assert_eq!(grid.point_count(), 64);
        assert!(grid.points().windows(2).all(|pair| pair[0] < pair[1]));
        assert!((extent.mean - 0.8).abs() < 1.0e-12);
        assert!((extent.rms - 0.864_098_759_787_714_8).abs() < 1.0e-12);
        assert!((extent.max - 1.2).abs() < 1.0e-12);
    }

    #[test]
    fn bound_state_clamps_iteration_and_mixing() {
        let grid = RadialGrid::from_extent(RadialExtent::new(0.5, 0.7, 1.0), 8, 0.03);
        let state = BoundStateSolverState::new(grid, 0, -2.0, -4.0);

        assert_eq!(state.iteration_limit(), 1);
        assert_eq!(state.mixing_parameter(), 1.0);
        assert!((state.muffin_tin_radius() - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn complex_energy_state_sanitizes_non_finite_values() {
        let grid = RadialGrid::from_extent(RadialExtent::new(1.0, 1.1, 1.4), 16, 0.02);
        let state = ComplexEnergySolverState::new(
            grid,
            Complex64::new(f64::NAN, f64::INFINITY),
            f64::NAN,
            0,
        );

        assert_eq!(state.energy(), Complex64::new(0.0, 0.0));
        assert_eq!(state.max_wave_number(), 1.0e-4);
        assert_eq!(state.channel_count_hint(), 1);
    }
}
