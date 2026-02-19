use crate::support::common::xx::xx;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScfdatPlanInput {
    pub niter: i32,
    pub norb: usize,
    pub norbsc: usize,
    pub testy: f64,
    pub rap: [f64; 2],
    pub teste: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScfdatPlan {
    pub niter_abs: usize,
    pub netir: usize,
    pub requires_schmidt: bool,
    pub test1: f64,
    pub test2: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitalSelectionState {
    pub current_j: usize,
    pub ind: i32,
    pub nter: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitalSelectionResult {
    pub next_j: usize,
    pub next_ind: i32,
    pub converged: bool,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum ScfdatError {
    #[error("norb must be >= 1")]
    InvalidNorb,
    #[error("rap components must be non-zero")]
    InvalidRap,
    #[error("testy and teste must be > 0")]
    InvalidThresholds,
    #[error("input length mismatch for {name}: need at least {need}, got {got}")]
    LengthMismatch {
        name: &'static str,
        need: usize,
        got: usize,
    },
    #[error("dr[{index}] must be > 0, got {value}")]
    NonPositiveRadius { index: usize, value: f64 },
    #[error("dr grid must be strictly increasing")]
    NonMonotonicRadiusGrid,
    #[error("matrix row mismatch for {name}[{index}]: need at least {need}, got {got}")]
    RowLengthMismatch {
        name: &'static str,
        index: usize,
        need: usize,
        got: usize,
    },
}

pub fn scfdat_plan(input: ScfdatPlanInput) -> Result<ScfdatPlan, ScfdatError> {
    if input.norb == 0 {
        return Err(ScfdatError::InvalidNorb);
    }
    if input.rap[0] == 0.0 || input.rap[1] == 0.0 {
        return Err(ScfdatError::InvalidRap);
    }
    if input.testy <= 0.0 || input.teste <= 0.0 {
        return Err(ScfdatError::InvalidThresholds);
    }

    let niter_abs = input.niter.unsigned_abs() as usize;
    Ok(ScfdatPlan {
        niter_abs,
        netir: niter_abs.saturating_mul(input.norb),
        requires_schmidt: input.niter < 0,
        test1: input.testy / input.rap[0],
        test2: input.testy / input.rap[1],
    })
}

pub fn select_next_orbital(
    state: OrbitalSelectionState,
    scw: &[f64],
    sce: &[f64],
    norbsc: usize,
    testy: f64,
    teste: f64,
) -> Result<OrbitalSelectionResult, ScfdatError> {
    if norbsc == 0 {
        return Err(ScfdatError::InvalidNorb);
    }
    ensure_len("scw", scw.len(), norbsc)?;
    ensure_len("sce", sce.len(), norbsc)?;

    if state.nter < norbsc || (state.ind < 0 && state.current_j < norbsc) {
        return Ok(OrbitalSelectionResult {
            next_j: state.current_j + 1,
            next_ind: state.ind,
            converged: false,
        });
    }

    let (j_scw, scw_max) = max_abs_index(scw, norbsc);
    if scw_max > testy {
        return Ok(OrbitalSelectionResult {
            next_j: j_scw,
            next_ind: 1,
            converged: false,
        });
    }

    let (j_sce, sce_max) = max_abs_index(sce, norbsc);
    if sce_max >= teste {
        return Ok(OrbitalSelectionResult {
            next_j: j_sce,
            next_ind: 1,
            converged: false,
        });
    }

    if state.ind < 0 {
        return Ok(OrbitalSelectionResult {
            next_j: state.current_j,
            next_ind: state.ind,
            converged: true,
        });
    }

    Ok(OrbitalSelectionResult {
        next_j: 1,
        next_ind: -1,
        converged: false,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn fix_atomic_quantities(
    dr: &[f64],
    vcoul: &mut [f64],
    srho: &mut [f64],
    dmag: &mut [f64],
    srhovl: &mut [f64],
    dgc0: &mut [f64],
    dpc0: &mut [f64],
    dgc: &mut [Vec<f64>],
    dpc: &mut [Vec<f64>],
) -> Result<(), ScfdatError> {
    let n = dr.len();
    if n == 0 {
        return Err(ScfdatError::InvalidNorb);
    }

    ensure_len("vcoul", vcoul.len(), n)?;
    ensure_len("srho", srho.len(), n)?;
    ensure_len("dmag", dmag.len(), n)?;
    ensure_len("srhovl", srhovl.len(), n)?;
    ensure_len("dgc0", dgc0.len(), n)?;
    ensure_len("dpc0", dpc0.len(), n)?;

    let mut row = 0usize;
    while row < dgc.len() {
        if dgc[row].len() < n {
            return Err(ScfdatError::RowLengthMismatch {
                name: "dgc",
                index: row + 1,
                need: n,
                got: dgc[row].len(),
            });
        }
        row += 1;
    }

    let mut row2 = 0usize;
    while row2 < dpc.len() {
        if dpc[row2].len() < n {
            return Err(ScfdatError::RowLengthMismatch {
                name: "dpc",
                index: row2 + 1,
                need: n,
                got: dpc[row2].len(),
            });
        }
        row2 += 1;
    }

    let mut xorg = vec![0.0_f64; n];
    let mut xnew = vec![0.0_f64; n];

    let mut i = 0usize;
    while i < n {
        if dr[i] <= 0.0 {
            return Err(ScfdatError::NonPositiveRadius {
                index: i,
                value: dr[i],
            });
        }
        if i > 0 && dr[i] <= dr[i - 1] {
            return Err(ScfdatError::NonMonotonicRadiusGrid);
        }
        xorg[i] = dr[i].ln();
        xnew[i] = xx((i + 1) as i32);
        i += 1;
    }

    remap_vector(&xorg, &xnew, dgc0);
    remap_vector(&xorg, &xnew, dpc0);

    let mut row3 = 0usize;
    while row3 < dgc.len() {
        remap_vector(&xorg, &xnew, &mut dgc[row3]);
        row3 += 1;
    }

    let mut row4 = 0usize;
    while row4 < dpc.len() {
        remap_vector(&xorg, &xnew, &mut dpc[row4]);
        row4 += 1;
    }

    remap_vector(&xorg, &xnew, vcoul);
    remap_vector(&xorg, &xnew, dmag);
    remap_vector(&xorg, &xnew, srho);
    remap_vector(&xorg, &xnew, srhovl);

    Ok(())
}

fn remap_vector(xorg: &[f64], xnew: &[f64], values: &mut [f64]) {
    let mut remapped = vec![0.0_f64; values.len()];

    let mut i = 0usize;
    while i < values.len() {
        remapped[i] = linear_interp(xorg, values, xnew[i]);
        i += 1;
    }

    values.copy_from_slice(&remapped);
}

fn linear_interp(x: &[f64], y: &[f64], xq: f64) -> f64 {
    if xq <= x[0] {
        return y[0];
    }

    let last = x.len() - 1;
    if xq >= x[last] {
        return y[last];
    }

    let mut low = 0usize;
    let mut high = last;
    while high - low > 1 {
        let mid = (low + high) / 2;
        if x[mid] <= xq {
            low = mid;
        } else {
            high = mid;
        }
    }

    let x0 = x[low];
    let x1 = x[high];
    let y0 = y[low];
    let y1 = y[high];

    let t = (xq - x0) / (x1 - x0);
    y0 + t * (y1 - y0)
}

fn max_abs_index(values: &[f64], count: usize) -> (usize, f64) {
    let mut max_value = 0.0_f64;
    let mut max_index = 1usize;

    let mut i = 0usize;
    while i < count {
        let value = values[i].abs();
        if value > max_value {
            max_value = value;
            max_index = i + 1;
        }
        i += 1;
    }

    (max_index, max_value)
}

fn ensure_len(name: &'static str, got: usize, need: usize) -> Result<(), ScfdatError> {
    if got < need {
        return Err(ScfdatError::LengthMismatch { name, need, got });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        OrbitalSelectionState, ScfdatPlanInput, fix_atomic_quantities, scfdat_plan,
        select_next_orbital,
    };
    use crate::support::common::xx::xx;

    #[test]
    fn scfdat_plan_matches_iteration_budget_formula() {
        let plan = scfdat_plan(ScfdatPlanInput {
            niter: 40,
            norb: 6,
            norbsc: 6,
            testy: 1.0e-5,
            rap: [100.0, 10.0],
            teste: 5.0e-6,
        })
        .expect("plan should succeed");

        assert_eq!(plan.niter_abs, 40);
        assert_eq!(plan.netir, 240);
        assert!(!plan.requires_schmidt);
        assert!((plan.test1 - 1.0e-7).abs() <= 1.0e-12);
        assert!((plan.test2 - 1.0e-6).abs() <= 1.0e-12);
    }

    #[test]
    fn select_next_orbital_switches_to_convergence_mode() {
        let result = select_next_orbital(
            OrbitalSelectionState {
                current_j: 3,
                ind: 1,
                nter: 10,
            },
            &[1.0e-8, 1.0e-8, 1.0e-8],
            &[1.0e-8, 1.0e-8, 1.0e-8],
            3,
            1.0e-5,
            5.0e-6,
        )
        .expect("selection should succeed");

        assert_eq!(result.next_j, 1);
        assert_eq!(result.next_ind, -1);
        assert!(!result.converged);
    }

    #[test]
    fn fix_atomic_quantities_is_identity_on_native_log_grid() {
        let n = 16usize;
        let mut dr = vec![0.0_f64; n];
        let mut i = 0usize;
        while i < n {
            dr[i] = xx((i + 1) as i32).exp();
            i += 1;
        }

        let mut vcoul: Vec<f64> = (0..n).map(|idx| idx as f64).collect();
        let mut srho: Vec<f64> = (0..n).map(|idx| (idx as f64) * 2.0).collect();
        let mut dmag: Vec<f64> = (0..n).map(|idx| (idx as f64) * 3.0).collect();
        let mut srhovl: Vec<f64> = (0..n).map(|idx| (idx as f64) * 4.0).collect();
        let mut dgc0: Vec<f64> = (0..n).map(|idx| (idx as f64) * 5.0).collect();
        let mut dpc0: Vec<f64> = (0..n).map(|idx| (idx as f64) * 6.0).collect();
        let mut dgc = vec![
            (0..n).map(|idx| idx as f64 + 1.0).collect::<Vec<_>>(),
            (0..n).map(|idx| idx as f64 + 2.0).collect::<Vec<_>>(),
        ];
        let mut dpc = vec![
            (0..n).map(|idx| idx as f64 + 3.0).collect::<Vec<_>>(),
            (0..n).map(|idx| idx as f64 + 4.0).collect::<Vec<_>>(),
        ];

        let original_vcoul = vcoul.clone();
        let original_dgc_row = dgc[0].clone();

        fix_atomic_quantities(
            &dr,
            &mut vcoul,
            &mut srho,
            &mut dmag,
            &mut srhovl,
            &mut dgc0,
            &mut dpc0,
            &mut dgc,
            &mut dpc,
        )
        .expect("native grid interpolation should succeed");

        assert!(
            vcoul
                .iter()
                .zip(original_vcoul.iter())
                .all(|(lhs, rhs)| (lhs - rhs).abs() <= 1.0e-12)
        );
        assert!(
            dgc[0]
                .iter()
                .zip(original_dgc_row.iter())
                .all(|(lhs, rhs)| (lhs - rhs).abs() <= 1.0e-12)
        );
    }
}
