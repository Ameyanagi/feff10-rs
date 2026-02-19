const THIRD: f64 = 1.0 / 3.0;
const FA: f64 = 1.919_158_292_677_512_9;

pub type VbhFn = dyn Fn(f64, f64) -> f64;
pub type EdpFn = dyn Fn(f64, f64) -> f64;

#[derive(Debug)]
pub struct VldaInput<'a> {
    pub ia: usize,
    pub xnval: &'a [f64],
    pub ilast: i32,
    pub idfock: i32,
}

#[derive(Debug)]
pub struct VldaState<'a> {
    pub cg: &'a [Vec<f64>],
    pub cp: &'a [Vec<f64>],
    pub xnel: &'a [f64],
    pub nmax: &'a [usize],
    pub norb: usize,
    pub idim: usize,
    pub dr: &'a [f64],
    pub cl: f64,
    pub srho: &'a mut [f64],
    pub srhovl: &'a mut [f64],
    pub vtrho: &'a mut [f64],
    pub dv: &'a mut [f64],
    pub av: &'a mut [f64],
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum VldaError {
    #[error("idfock must be one of 1, 2, 5, 6, got {0}")]
    InvalidIdfock(i32),
    #[error("cl must be non-zero")]
    ZeroCl,
    #[error("norb={norb} exceeds available orbital arrays")]
    InvalidOrbitalCount { norb: usize },
    #[error("input length mismatch for {name}: need at least {need}, got {got}")]
    LengthMismatch {
        name: &'static str,
        need: usize,
        got: usize,
    },
    #[error("radial grid dr[{index}] must be > 0, got {value}")]
    NonPositiveRadius { index: usize, value: f64 },
}

pub fn vlda(
    input: &VldaInput<'_>,
    state: &mut VldaState<'_>,
    vbh: &VbhFn,
    edp: &EdpFn,
) -> Result<(), VldaError> {
    let _ = input.ia;

    if state.cl == 0.0 {
        return Err(VldaError::ZeroCl);
    }
    if !matches!(input.idfock, 1 | 2 | 5 | 6) {
        return Err(VldaError::InvalidIdfock(input.idfock));
    }
    validate_lengths(input, state)?;

    let mut i = 0usize;
    while i < state.idim {
        state.srho[i] = 0.0;
        state.srhovl[i] = 0.0;
        i += 1;
    }

    let mut j = 0usize;
    while j < state.norb {
        let a = state.xnel[j];
        let b = input.xnval[j];
        let upper = state.nmax[j].min(state.idim);

        let mut i_inner = 0usize;
        while i_inner < upper {
            let density = state.cg[j][i_inner] * state.cg[j][i_inner]
                + state.cp[j][i_inner] * state.cp[j][i_inner];
            state.srho[i_inner] += a * density;
            state.srhovl[i_inner] += b * density;
            i_inner += 1;
        }

        j += 1;
    }

    let mut idx = 0usize;
    while idx < state.idim {
        if state.dr[idx] <= 0.0 {
            return Err(VldaError::NonPositiveRadius {
                index: idx,
                value: state.dr[idx],
            });
        }

        let radius_sq = state.dr[idx] * state.dr[idx];
        let rho = state.srho[idx] / radius_sq;
        let rhoc = match input.idfock {
            5 => state.srhovl[idx] / radius_sq,
            6 => (state.srho[idx] - state.srhovl[idx]) / radius_sq,
            1 => 0.0,
            2 => state.srho[idx] / radius_sq,
            _ => unreachable!(),
        };

        if rho > 0.0 {
            let rs = (rho / 3.0).powf(-THIRD);
            let mut rsc = 101.0;
            if rhoc > 0.0 {
                rsc = (rhoc / 3.0).powf(-THIRD);
            }

            let vxcvl = match input.idfock {
                5 | 2 => vbh(rsc, 1.0),
                6 => {
                    let vvbh = vbh(rs, 1.0);
                    let xf = FA / rs;
                    let vdh = edp(rsc, xf);
                    vvbh - vdh
                }
                1 => 0.0,
                _ => unreachable!(),
            };

            if input.ilast > 0 {
                state.vtrho[idx] += vxcvl * state.srho[idx];
            }
            if idx == 0 {
                state.av[1] += vxcvl / state.cl;
            }
            state.dv[idx] += vxcvl / state.cl;
        }

        idx += 1;
    }

    Ok(())
}

fn validate_lengths(input: &VldaInput<'_>, state: &VldaState<'_>) -> Result<(), VldaError> {
    if state.norb == 0
        || state.norb > state.cg.len()
        || state.norb > state.cp.len()
        || state.norb > state.xnel.len()
        || state.norb > state.nmax.len()
        || state.norb > input.xnval.len()
    {
        return Err(VldaError::InvalidOrbitalCount { norb: state.norb });
    }

    ensure_len("dr", state.dr.len(), state.idim)?;
    ensure_len("srho", state.srho.len(), state.idim)?;
    ensure_len("srhovl", state.srhovl.len(), state.idim)?;
    ensure_len("vtrho", state.vtrho.len(), state.idim)?;
    ensure_len("dv", state.dv.len(), state.idim)?;
    ensure_len("av", state.av.len(), 2)?;

    let mut i = 0usize;
    while i < state.norb {
        ensure_len("cg row", state.cg[i].len(), state.idim)?;
        ensure_len("cp row", state.cp[i].len(), state.idim)?;
        i += 1;
    }

    Ok(())
}

fn ensure_len(name: &'static str, got: usize, need: usize) -> Result<(), VldaError> {
    if got < need {
        return Err(VldaError::LengthMismatch { name, need, got });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{VldaError, VldaInput, VldaState, vlda};

    #[test]
    fn vlda_accumulates_density_and_updates_potential_for_idfock_5() {
        let cg = vec![vec![1.0, 0.5, 0.0], vec![0.5, 0.5, 0.0]];
        let cp = vec![vec![0.0, 0.0, 0.0], vec![0.0, 0.0, 0.0]];
        let xnel = vec![2.0, 1.0];
        let nmax = vec![2, 2];

        let mut srho = vec![0.0; 3];
        let mut srhovl = vec![0.0; 3];
        let mut vtrho = vec![0.0; 3];
        let mut dv = vec![0.0; 3];
        let mut av = vec![0.0; 3];

        let mut state = VldaState {
            cg: &cg,
            cp: &cp,
            xnel: &xnel,
            nmax: &nmax,
            norb: 2,
            idim: 3,
            dr: &[1.0, 2.0, 3.0],
            cl: 2.0,
            srho: &mut srho,
            srhovl: &mut srhovl,
            vtrho: &mut vtrho,
            dv: &mut dv,
            av: &mut av,
        };

        vlda(
            &VldaInput {
                ia: 1,
                xnval: &[1.0, 0.5],
                ilast: 1,
                idfock: 5,
            },
            &mut state,
            &|rsc, _| 1.0 / (1.0 + rsc),
            &|_, _| 0.0,
        )
        .expect("vlda should succeed");

        assert!(state.srho[0] > 0.0);
        assert!(state.srhovl[0] > 0.0);
        assert!(state.dv[0] > 0.0);
        assert!(state.vtrho[0] > 0.0);
        assert!(state.av[1] > 0.0);
    }

    #[test]
    fn vlda_supports_idfock_6_core_valence_subtraction() {
        let cg = vec![vec![1.0, 0.0, 0.0]];
        let cp = vec![vec![0.0, 0.0, 0.0]];
        let xnel = vec![1.0];
        let nmax = vec![1];

        let mut srho = vec![0.0; 3];
        let mut srhovl = vec![0.0; 3];
        let mut vtrho = vec![0.0; 3];
        let mut dv = vec![0.0; 3];
        let mut av = vec![0.0; 3];

        let mut state = VldaState {
            cg: &cg,
            cp: &cp,
            xnel: &xnel,
            nmax: &nmax,
            norb: 1,
            idim: 3,
            dr: &[1.0, 2.0, 3.0],
            cl: 3.0,
            srho: &mut srho,
            srhovl: &mut srhovl,
            vtrho: &mut vtrho,
            dv: &mut dv,
            av: &mut av,
        };

        vlda(
            &VldaInput {
                ia: 1,
                xnval: &[0.2],
                ilast: 0,
                idfock: 6,
            },
            &mut state,
            &|rs, _| 2.0 / (1.0 + rs),
            &|rsc, xf| 0.5 * (rsc + xf),
        )
        .expect("idfock=6 should succeed");

        assert!(state.dv[0].is_finite());
        assert!(state.dv[0] != 0.0);
    }

    #[test]
    fn vlda_rejects_unknown_idfock_mode() {
        let cg = vec![vec![1.0, 0.0, 0.0]];
        let cp = vec![vec![0.0, 0.0, 0.0]];
        let xnel = vec![1.0];
        let nmax = vec![1];

        let mut srho = vec![0.0; 3];
        let mut srhovl = vec![0.0; 3];
        let mut vtrho = vec![0.0; 3];
        let mut dv = vec![0.0; 3];
        let mut av = vec![0.0; 3];

        let mut state = VldaState {
            cg: &cg,
            cp: &cp,
            xnel: &xnel,
            nmax: &nmax,
            norb: 1,
            idim: 3,
            dr: &[1.0, 2.0, 3.0],
            cl: 1.0,
            srho: &mut srho,
            srhovl: &mut srhovl,
            vtrho: &mut vtrho,
            dv: &mut dv,
            av: &mut av,
        };

        let error = vlda(
            &VldaInput {
                ia: 1,
                xnval: &[0.0],
                ilast: 0,
                idfock: 9,
            },
            &mut state,
            &|_, _| 0.0,
            &|_, _| 0.0,
        )
        .expect_err("invalid idfock should fail");

        assert_eq!(error, VldaError::InvalidIdfock(9));
    }
}
