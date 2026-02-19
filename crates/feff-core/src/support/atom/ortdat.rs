pub type DsordfFn = dyn FnMut(i32, i32, i32, i32, f64) -> f64;

#[derive(Debug, Clone)]
pub struct OrtdatState {
    pub cg: Vec<Vec<f64>>,
    pub cp: Vec<Vec<f64>>,
    pub bg: Vec<Vec<f64>>,
    pub bp: Vec<Vec<f64>>,
    pub fl: Vec<f64>,
    pub kap: Vec<i32>,
    pub nmax: Vec<usize>,
    pub norb: usize,
    pub ndor: usize,
    pub idim: usize,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum OrtdatError {
    #[error("norb={norb} exceeds available orbital arrays")]
    InvalidOrbitalCount { norb: usize },
    #[error("ia must be in [-norb, norb], got {ia}")]
    InvalidIa { ia: i32 },
    #[error("row length mismatch for {name}[{index}]: need {need}, got {got}")]
    RowLengthMismatch {
        name: &'static str,
        index: usize,
        need: usize,
        got: usize,
    },
    #[error("nmax({index})={value} exceeds idim={idim}")]
    NmaxOutOfRange {
        index: usize,
        value: usize,
        idim: usize,
    },
    #[error("normalization integral must be > 0 for orbital {index}, got {value}")]
    NonPositiveNorm { index: usize, value: f64 },
}

pub fn ortdat(ia: i32, state: &mut OrtdatState, dsordf: &mut DsordfFn) -> Result<(), OrtdatError> {
    validate_state(state, ia)?;

    let mut m = state.norb;
    let mut l = ia.max(1) as usize;

    loop {
        if ia <= 0 {
            m = l;
            l += 1;
            if l > state.norb {
                break;
            }
        }

        process_one_orbital(l, m, state, dsordf)?;

        if ia > 0 {
            break;
        }
    }

    Ok(())
}

fn process_one_orbital(
    l_one_based: usize,
    m: usize,
    state: &mut OrtdatState,
    dsordf: &mut DsordfFn,
) -> Result<(), OrtdatError> {
    let l_idx = l_one_based - 1;

    let mut dg = vec![0.0_f64; state.idim];
    let mut dp = vec![0.0_f64; state.idim];
    let mut ag = vec![0.0_f64; state.ndor];
    let mut ap = vec![0.0_f64; state.ndor];

    let mut maxl = state.nmax[l_idx];
    dg[..maxl].copy_from_slice(&state.cg[l_idx][..maxl]);
    dp[..maxl].copy_from_slice(&state.cp[l_idx][..maxl]);
    ag[..state.ndor].copy_from_slice(&state.bg[l_idx][..state.ndor]);
    ap[..state.ndor].copy_from_slice(&state.bp[l_idx][..state.ndor]);

    let mut j = 1usize;
    while j <= m {
        let j_idx = j - 1;
        if j != l_one_based && state.kap[j_idx] == state.kap[l_idx] {
            let max0 = state.nmax[j_idx];
            let a = dsordf(j as i32, j as i32, 0, 3, state.fl[l_idx]);

            let mut i = 0usize;
            while i < max0 {
                dg[i] -= a * state.cg[j_idx][i];
                dp[i] -= a * state.cp[j_idx][i];
                i += 1;
            }

            let mut n = 0usize;
            while n < state.ndor {
                ag[n] -= a * state.bg[j_idx][n];
                ap[n] -= a * state.bp[j_idx][n];
                n += 1;
            }

            if max0 > maxl {
                maxl = max0;
            }
        }
        j += 1;
    }

    state.nmax[l_idx] = maxl;
    let norm = dsordf(l_one_based as i32, maxl as i32, 0, 4, state.fl[l_idx]);
    if norm <= 0.0 {
        return Err(OrtdatError::NonPositiveNorm {
            index: l_one_based,
            value: norm,
        });
    }

    let scale = norm.sqrt();
    let mut i = 0usize;
    while i < maxl {
        state.cg[l_idx][i] = dg[i] / scale;
        state.cp[l_idx][i] = dp[i] / scale;
        i += 1;
    }

    let mut n = 0usize;
    while n < state.ndor {
        state.bg[l_idx][n] = ag[n] / scale;
        state.bp[l_idx][n] = ap[n] / scale;
        n += 1;
    }

    Ok(())
}

fn validate_state(state: &OrtdatState, ia: i32) -> Result<(), OrtdatError> {
    if state.norb == 0
        || state.norb > state.cg.len()
        || state.norb > state.cp.len()
        || state.norb > state.bg.len()
        || state.norb > state.bp.len()
        || state.norb > state.fl.len()
        || state.norb > state.kap.len()
        || state.norb > state.nmax.len()
    {
        return Err(OrtdatError::InvalidOrbitalCount { norb: state.norb });
    }

    if ia.unsigned_abs() as usize > state.norb {
        return Err(OrtdatError::InvalidIa { ia });
    }

    let mut i = 0usize;
    while i < state.norb {
        if state.nmax[i] > state.idim {
            return Err(OrtdatError::NmaxOutOfRange {
                index: i + 1,
                value: state.nmax[i],
                idim: state.idim,
            });
        }
        if state.cg[i].len() < state.idim {
            return Err(OrtdatError::RowLengthMismatch {
                name: "cg",
                index: i + 1,
                need: state.idim,
                got: state.cg[i].len(),
            });
        }
        if state.cp[i].len() < state.idim {
            return Err(OrtdatError::RowLengthMismatch {
                name: "cp",
                index: i + 1,
                need: state.idim,
                got: state.cp[i].len(),
            });
        }
        if state.bg[i].len() < state.ndor {
            return Err(OrtdatError::RowLengthMismatch {
                name: "bg",
                index: i + 1,
                need: state.ndor,
                got: state.bg[i].len(),
            });
        }
        if state.bp[i].len() < state.ndor {
            return Err(OrtdatError::RowLengthMismatch {
                name: "bp",
                index: i + 1,
                need: state.ndor,
                got: state.bp[i].len(),
            });
        }
        i += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{OrtdatError, OrtdatState, ortdat};

    fn make_state() -> OrtdatState {
        OrtdatState {
            cg: vec![vec![1.0, 0.0, 0.0], vec![1.0, 1.0, 0.0]],
            cp: vec![vec![0.0, 0.0, 0.0], vec![0.0, 0.0, 0.0]],
            bg: vec![vec![1.0], vec![1.0]],
            bp: vec![vec![0.0], vec![0.0]],
            fl: vec![0.0, 0.0],
            kap: vec![1, 1],
            nmax: vec![2, 2],
            norb: 2,
            ndor: 1,
            idim: 3,
        }
    }

    #[test]
    fn ortdat_orthogonalizes_single_target_orbital() {
        let mut state = make_state();
        let mut dsordf = |_: i32, _: i32, _: i32, jnd: i32, _: f64| {
            if jnd == 3 { 1.0 } else { 4.0 }
        };

        ortdat(2, &mut state, &mut dsordf).expect("ortdat should succeed");

        assert!((state.cg[1][0] - 0.0).abs() <= 1.0e-12);
        assert!((state.cg[1][1] - 0.5).abs() <= 1.0e-12);
        assert!((state.bg[1][0] - 0.0).abs() <= 1.0e-12);
    }

    #[test]
    fn ortdat_handles_full_schmidt_mode_for_non_positive_ia() {
        let mut state = OrtdatState {
            cg: vec![
                vec![1.0, 0.0, 0.0],
                vec![1.0, 1.0, 0.0],
                vec![1.0, 1.0, 1.0],
            ],
            cp: vec![vec![0.0, 0.0, 0.0]; 3],
            bg: vec![vec![1.0], vec![1.0], vec![1.0]],
            bp: vec![vec![0.0], vec![0.0], vec![0.0]],
            fl: vec![0.0, 0.0, 0.0],
            kap: vec![1, 1, 1],
            nmax: vec![2, 2, 3],
            norb: 3,
            ndor: 1,
            idim: 3,
        };
        let mut dsordf = |_: i32, _: i32, _: i32, jnd: i32, _: f64| {
            if jnd == 3 { 0.5 } else { 1.0 }
        };

        ortdat(0, &mut state, &mut dsordf).expect("ortdat should succeed");
        assert!(state.cg[1][0].abs() < 1.0);
        assert!(state.cg[2][0].abs() < 1.0);
    }

    #[test]
    fn ortdat_rejects_non_positive_norm() {
        let mut state = make_state();
        let mut dsordf = |_: i32, _: i32, _: i32, jnd: i32, _: f64| {
            if jnd == 4 { 0.0 } else { 1.0 }
        };

        let error = ortdat(2, &mut state, &mut dsordf).expect_err("zero norm should fail");
        assert_eq!(
            error,
            OrtdatError::NonPositiveNorm {
                index: 2,
                value: 0.0
            }
        );
    }
}
