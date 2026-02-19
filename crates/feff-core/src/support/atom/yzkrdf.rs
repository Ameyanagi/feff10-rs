use super::aprdev::aprdev;
use super::yzkteg::{YzktegError, YzktegInput, yzkteg};

#[derive(Debug, Clone)]
pub struct YzkrdfContext {
    pub cg: Vec<Vec<f64>>,
    pub cp: Vec<Vec<f64>>,
    pub bg: Vec<Vec<f64>>,
    pub bp: Vec<Vec<f64>>,
    pub fl: Vec<f64>,
    pub nmax: Vec<usize>,
    pub dr: Vec<f64>,
    pub hx: f64,
    pub ndor: usize,
    pub idim: usize,
    pub nem: i32,
}

#[derive(Debug, Clone)]
pub enum YzkrdfSource<'a> {
    Orbitals {
        i: usize,
        j: usize,
    },
    Prebuilt {
        id: usize,
        dg: &'a [f64],
        ag: &'a [f64],
    },
}

#[derive(Debug, Clone)]
pub struct YzkrdfInput<'a> {
    pub source: YzkrdfSource<'a>,
    pub k: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct YzkrdfOutput {
    pub dg: Vec<f64>,
    pub ag: Vec<f64>,
    pub dp: Vec<f64>,
    pub chg: Vec<f64>,
    pub ap: f64,
    pub id: usize,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum YzkrdfError {
    #[error("ndor must be >= 1")]
    InvalidNdor,
    #[error("idim must be >= 5, got {0}")]
    InvalidIdim(usize),
    #[error("context length mismatch for {name}: need at least {need}, got {got}")]
    LengthMismatch {
        name: &'static str,
        need: usize,
        got: usize,
    },
    #[error("orbital index {index} is outside 1..={available}")]
    OrbitalOutOfRange { index: usize, available: usize },
    #[error("requested prebuilt id={id} exceeds idim={idim}")]
    PrebuiltIdOutOfRange { id: usize, idim: usize },
    #[error(transparent)]
    Yzkteg(#[from] YzktegError),
}

pub fn yzkrdf(
    input: &YzkrdfInput<'_>,
    context: &YzkrdfContext,
) -> Result<YzkrdfOutput, YzkrdfError> {
    validate_context(context)?;

    let mut dg = vec![0.0_f64; context.idim];
    let mut ag = vec![0.0_f64; context.ndor];
    let (id, ap) = match &input.source {
        YzkrdfSource::Orbitals { i, j } => {
            let i_idx = i.saturating_sub(1);
            let j_idx = j.saturating_sub(1);
            validate_orbital(i_idx, context.cg.len())?;
            validate_orbital(j_idx, context.cg.len())?;
            validate_orbital(i_idx, context.cp.len())?;
            validate_orbital(j_idx, context.cp.len())?;
            validate_orbital(i_idx, context.bg.len())?;
            validate_orbital(j_idx, context.bg.len())?;
            validate_orbital(i_idx, context.bp.len())?;
            validate_orbital(j_idx, context.bp.len())?;
            validate_orbital(i_idx, context.fl.len())?;
            validate_orbital(j_idx, context.fl.len())?;
            validate_orbital(i_idx, context.nmax.len())?;
            validate_orbital(j_idx, context.nmax.len())?;

            let id = context.nmax[i_idx]
                .min(context.nmax[j_idx])
                .min(context.idim);
            let mut m = 0usize;
            while m < id {
                dg[m] = if context.nem == 0 {
                    context.cg[i_idx][m] * context.cg[j_idx][m]
                        + context.cp[i_idx][m] * context.cp[j_idx][m]
                } else {
                    context.cg[i_idx][m] * context.cp[j_idx][m]
                };
                m += 1;
            }

            let mut l = 0usize;
            while l < context.ndor {
                ag[l] = if context.nem == 0 {
                    aprdev(&context.bg[i_idx], &context.bg[j_idx], l + 1)
                        + aprdev(&context.bp[i_idx], &context.bp[j_idx], l + 1)
                } else {
                    aprdev(&context.bg[i_idx], &context.bp[j_idx], l + 1)
                };
                l += 1;
            }

            (id, context.fl[i_idx] + context.fl[j_idx])
        }
        YzkrdfSource::Prebuilt {
            id,
            dg: src_dg,
            ag: src_ag,
        } => {
            if *id > context.idim {
                return Err(YzkrdfError::PrebuiltIdOutOfRange {
                    id: *id,
                    idim: context.idim,
                });
            }
            ensure_len("prebuilt dg", src_dg.len(), context.idim)?;
            ensure_len("prebuilt ag", src_ag.len(), context.ndor)?;

            dg.copy_from_slice(&src_dg[..context.idim]);
            ag.copy_from_slice(&src_ag[..context.ndor]);

            (*id, input.k as f64 + 2.0)
        }
    };

    let integrated = yzkteg(&YzktegInput {
        f: &dg,
        af: &ag,
        dr: &context.dr,
        ap,
        h: context.hx,
        k: input.k,
        nd: context.ndor,
        np: id,
        idim: context.idim,
    })?;

    Ok(YzkrdfOutput {
        dg: integrated.yk,
        ag: integrated.yk_dev,
        dp: integrated.zk,
        chg: integrated.zk_dev,
        ap: integrated.ap,
        id,
    })
}

fn validate_context(context: &YzkrdfContext) -> Result<(), YzkrdfError> {
    if context.ndor == 0 {
        return Err(YzkrdfError::InvalidNdor);
    }
    if context.idim < 5 {
        return Err(YzkrdfError::InvalidIdim(context.idim));
    }
    ensure_len("dr", context.dr.len(), context.idim)?;

    let norb = context.cg.len();
    if context.cp.len() < norb
        || context.bg.len() < norb
        || context.bp.len() < norb
        || context.fl.len() < norb
        || context.nmax.len() < norb
    {
        return Err(YzkrdfError::LengthMismatch {
            name: "orbital tables",
            need: norb,
            got: context
                .cp
                .len()
                .min(context.bg.len())
                .min(context.bp.len())
                .min(context.fl.len())
                .min(context.nmax.len()),
        });
    }

    let mut idx = 0usize;
    while idx < norb {
        ensure_len("cg row", context.cg[idx].len(), context.idim)?;
        ensure_len("cp row", context.cp[idx].len(), context.idim)?;
        ensure_len("bg row", context.bg[idx].len(), context.ndor)?;
        ensure_len("bp row", context.bp[idx].len(), context.ndor)?;
        idx += 1;
    }

    Ok(())
}

fn ensure_len(name: &'static str, got: usize, need: usize) -> Result<(), YzkrdfError> {
    if got < need {
        return Err(YzkrdfError::LengthMismatch { name, need, got });
    }
    Ok(())
}

fn validate_orbital(index: usize, available: usize) -> Result<(), YzkrdfError> {
    if index >= available {
        return Err(YzkrdfError::OrbitalOutOfRange {
            index: index + 1,
            available,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{YzkrdfContext, YzkrdfInput, YzkrdfSource, yzkrdf};

    fn context(nem: i32) -> YzkrdfContext {
        YzkrdfContext {
            cg: vec![vec![1.0, 0.5, 0.25, 0.125, 0.0]],
            cp: vec![vec![0.0, 0.0, 0.0, 0.0, 0.0]],
            bg: vec![vec![0.0, 0.0, 0.0]],
            bp: vec![vec![0.0, 0.0, 0.0]],
            fl: vec![1.0],
            nmax: vec![5],
            dr: vec![0.8, 0.9, 1.0, 1.1, 1.2],
            hx: 0.04,
            ndor: 3,
            idim: 5,
            nem,
        }
    }

    #[test]
    fn yzkrdf_builds_orbital_source_when_nem_is_zero() {
        let output = yzkrdf(
            &YzkrdfInput {
                source: YzkrdfSource::Orbitals { i: 1, j: 1 },
                k: 0,
            },
            &context(0),
        )
        .expect("orbital path should succeed");

        assert!(output.dg.iter().any(|value| value.abs() > 1.0e-9));
        assert!(output.ap.is_finite());
        assert_eq!(output.id, 5);
    }

    #[test]
    fn yzkrdf_uses_prebuilt_source_for_i_le_zero_mode() {
        let source_dg = vec![0.0, 0.0, 0.0, 0.0, 0.0];
        let source_ag = vec![0.0, 0.0, 0.0];
        let output = yzkrdf(
            &YzkrdfInput {
                source: YzkrdfSource::Prebuilt {
                    id: 5,
                    dg: &source_dg,
                    ag: &source_ag,
                },
                k: 2,
            },
            &context(0),
        )
        .expect("prebuilt path should succeed");

        assert!(output.dg.iter().all(|value| value.abs() <= 1.0e-12));
        assert!(output.dp.iter().all(|value| value.abs() <= 1.0e-12));
        assert!(output.ap.abs() <= 1.0e-12);
    }

    #[test]
    fn yzkrdf_nem_exchange_path_can_zero_out_density_product() {
        let output = yzkrdf(
            &YzkrdfInput {
                source: YzkrdfSource::Orbitals { i: 1, j: 1 },
                k: 0,
            },
            &context(1),
        )
        .expect("exchange path should succeed");

        assert!(output.dg.iter().all(|value| value.abs() <= 1.0e-12));
        assert!(output.ap.is_finite());
    }
}
