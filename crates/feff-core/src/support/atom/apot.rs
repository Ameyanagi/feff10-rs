pub const XION_EPSILON: f64 = 1.0e-3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicPotentialPlan {
    pub effective_nohole: i32,
    pub free_atom_passes: usize,
    pub needs_core_hole_density: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreHoleMode {
    FrozenCoreOrbital,
    TransitionDensity,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum ApotError {
    #[error("input arrays must share the same non-zero length")]
    LengthMismatch,
    #[error("radial grid values must be > 0 at index {index}, got {value}")]
    NonPositiveRadius { index: usize, value: f64 },
}

pub fn plan_atomic_potentials(nohole: i32, xion: &[f64], nph: usize) -> AtomicPotentialPlan {
    let effective_nohole = if nohole == 2 { 0 } else { nohole };
    let limit = xion.len().min(nph.saturating_add(1));
    let has_ionicity = xion[..limit]
        .iter()
        .copied()
        .any(|value| value.abs() > XION_EPSILON);

    AtomicPotentialPlan {
        effective_nohole,
        free_atom_passes: if has_ionicity { 2 } else { 1 },
        needs_core_hole_density: effective_nohole > 0,
    }
}

pub fn relaxation_and_edge_energy(etfin: f64, etinit: f64, efrozn: f64) -> (f64, f64) {
    let erelax = -efrozn - (etfin - etinit);
    let mut emu = etfin - etinit;
    if emu <= 0.0 {
        emu = -efrozn;
    }
    (erelax, emu)
}

#[allow(clippy::too_many_arguments)]
pub fn scaled_core_hole_density(
    mode: CoreHoleMode,
    dr: &[f64],
    dgc0: &[f64],
    dpc0: &[f64],
    rho_iph0: &[f64],
    rhoval_iph0: &[f64],
    rho_nph1: &[f64],
    rhoval_nph1: &[f64],
) -> Result<Vec<f64>, ApotError> {
    let n = dr.len();
    if n == 0 {
        return Err(ApotError::LengthMismatch);
    }

    match mode {
        CoreHoleMode::FrozenCoreOrbital => {
            if dgc0.len() != n || dpc0.len() != n {
                return Err(ApotError::LengthMismatch);
            }
            let mut values = Vec::with_capacity(n);
            for index in 0..n {
                if dr[index] <= 0.0 {
                    return Err(ApotError::NonPositiveRadius {
                        index,
                        value: dr[index],
                    });
                }
                values.push(
                    (dgc0[index] * dgc0[index] + dpc0[index] * dpc0[index])
                        / (2.0 * dr[index] * dr[index]),
                );
            }
            Ok(values)
        }
        CoreHoleMode::TransitionDensity => {
            if rho_iph0.len() != n
                || rhoval_iph0.len() != n
                || rho_nph1.len() != n
                || rhoval_nph1.len() != n
            {
                return Err(ApotError::LengthMismatch);
            }
            let mut values = Vec::with_capacity(n);
            for index in 0..n {
                values.push(
                    (rho_iph0[index] - rhoval_iph0[index] - rho_nph1[index] + rhoval_nph1[index])
                        / 2.0,
                );
            }
            Ok(values)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ApotError, CoreHoleMode, XION_EPSILON, plan_atomic_potentials, relaxation_and_edge_energy,
        scaled_core_hole_density,
    };

    #[test]
    fn plan_atomic_potentials_tracks_ionicity_and_nohole_override() {
        let plan = plan_atomic_potentials(2, &[0.0, XION_EPSILON * 2.0], 1);
        assert_eq!(plan.effective_nohole, 0);
        assert_eq!(plan.free_atom_passes, 2);
        assert!(!plan.needs_core_hole_density);
    }

    #[test]
    fn relaxation_and_edge_energy_applies_negative_emu_fallback() {
        let (erelax, emu) = relaxation_and_edge_energy(10.0, 12.0, -3.5);
        assert!((erelax - 5.5).abs() <= 1.0e-12);
        assert!((emu - 3.5).abs() <= 1.0e-12);
    }

    #[test]
    fn frozen_core_density_matches_fortran_half_hole_scaling() {
        let values = scaled_core_hole_density(
            CoreHoleMode::FrozenCoreOrbital,
            &[1.0, 2.0],
            &[2.0, 4.0],
            &[0.0, 2.0],
            &[],
            &[],
            &[],
            &[],
        )
        .expect("valid frozen core density should succeed");
        assert!((values[0] - 2.0).abs() <= 1.0e-12);
        assert!((values[1] - 2.5).abs() <= 1.0e-12);
    }

    #[test]
    fn transition_density_matches_difference_formula() {
        let values = scaled_core_hole_density(
            CoreHoleMode::TransitionDensity,
            &[1.0, 2.0],
            &[],
            &[],
            &[10.0, 11.0],
            &[1.0, 2.0],
            &[6.0, 7.0],
            &[0.0, 1.0],
        )
        .expect("valid transition density should succeed");
        assert_eq!(values, vec![1.5, 1.5]);
    }

    #[test]
    fn frozen_mode_rejects_non_positive_radius() {
        let error = scaled_core_hole_density(
            CoreHoleMode::FrozenCoreOrbital,
            &[0.0],
            &[1.0],
            &[1.0],
            &[],
            &[],
            &[],
            &[],
        )
        .expect_err("zero radius should fail");
        assert_eq!(
            error,
            ApotError::NonPositiveRadius {
                index: 0,
                value: 0.0
            }
        );
    }
}
