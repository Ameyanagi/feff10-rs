use super::aprdev::aprdev;

#[derive(Debug, Clone)]
pub struct DsordfInput<'a> {
    pub n: i32,
    pub jnd: i32,
    pub a: f64,
    pub ndor: usize,
    pub hx: f64,
    pub dr: &'a [f64],
    pub dg: &'a [f64],
    pub dp: &'a [f64],
    pub ag: &'a [f64],
    pub ap: &'a [f64],
    pub cg_i: &'a [f64],
    pub cg_j: &'a [f64],
    pub cp_i: &'a [f64],
    pub cp_j: &'a [f64],
    pub bg_i: &'a [f64],
    pub bg_j: &'a [f64],
    pub bp_i: &'a [f64],
    pub bp_j: &'a [f64],
    pub fl_i: f64,
    pub fl_j: f64,
    pub nmax_i: usize,
    pub nmax_j: usize,
    pub integration_limit: usize,
    pub prebuilt_hg: Option<&'a [f64]>,
    pub prebuilt_chg: Option<&'a [f64]>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum DsordfError {
    #[error("jnd must be non-zero, got {0}")]
    InvalidMode(i32),
    #[error("ndor must be at least 1")]
    InvalidNdor,
    #[error("input length mismatch for {name}: need at least {need}, got {got}")]
    LengthMismatch {
        name: &'static str,
        need: usize,
        got: usize,
    },
    #[error("simpson integration requires odd number of radial points >= 3, got {0}")]
    InvalidRadialPointCount(usize),
    #[error("jnd>=5 requires prebuilt hg input")]
    MissingPrebuiltHg,
    #[error("origin integral denominator collapsed at series term {term_index}")]
    SingularOriginTerm { term_index: usize },
}

pub fn dsordf(input: &DsordfInput<'_>) -> Result<f64, DsordfError> {
    if input.jnd == 0 {
        return Err(DsordfError::InvalidMode(input.jnd));
    }
    if input.ndor == 0 {
        return Err(DsordfError::InvalidNdor);
    }
    ensure_length("dr", input.dr.len(), 1)?;
    ensure_length("ag", input.ag.len(), input.ndor)?;
    ensure_length("ap", input.ap.len(), input.ndor)?;

    let abs_jnd = input.jnd.abs();
    let mut chg = vec![0.0; input.ndor];
    let mut b;
    let mut hg;

    if abs_jnd == 1 {
        ensure_length("cg_i", input.cg_i.len(), 1)?;
        ensure_length("cg_j", input.cg_j.len(), 1)?;
        ensure_length("cp_i", input.cp_i.len(), 1)?;
        ensure_length("cp_j", input.cp_j.len(), 1)?;
        ensure_length("bg_i", input.bg_i.len(), input.ndor)?;
        ensure_length("bg_j", input.bg_j.len(), input.ndor)?;
        ensure_length("bp_i", input.bp_i.len(), input.ndor)?;
        ensure_length("bp_j", input.bp_j.len(), input.ndor)?;

        let max0 = input
            .nmax_i
            .min(input.nmax_j)
            .min(input.cg_i.len())
            .min(input.cg_j.len())
            .min(input.cp_i.len())
            .min(input.cp_j.len())
            .min(input.dr.len());
        validate_simpson_points(max0)?;

        hg = Vec::with_capacity(max0);
        for idx in 0..max0 {
            hg.push(input.cg_i[idx] * input.cg_j[idx] + input.cp_i[idx] * input.cp_j[idx]);
        }

        for l in 1..=input.ndor {
            chg[l - 1] = aprdev(input.bg_i, input.bg_j, l) + aprdev(input.bp_i, input.bp_j, l);
        }
        b = input.fl_i + input.fl_j;

        if input.jnd < 0 {
            ensure_length("dg", input.dg.len(), max0)?;
            for (idx, value) in hg.iter_mut().enumerate().take(max0) {
                *value *= input.dg[idx];
            }
            let previous = chg.clone();
            b += input.a;
            for l in 1..=input.ndor {
                chg[l - 1] = aprdev(&previous, input.ag, l);
            }
        }
    } else if abs_jnd == 2 {
        ensure_length("cg_i", input.cg_i.len(), 1)?;
        ensure_length("cp_j", input.cp_j.len(), 1)?;
        ensure_length("bg_i", input.bg_i.len(), input.ndor)?;
        ensure_length("bp_j", input.bp_j.len(), input.ndor)?;

        let max0 = input
            .nmax_i
            .min(input.nmax_j)
            .min(input.cg_i.len())
            .min(input.cp_j.len())
            .min(input.dr.len());
        validate_simpson_points(max0)?;

        hg = Vec::with_capacity(max0);
        for idx in 0..max0 {
            hg.push(input.cg_i[idx] * input.cp_j[idx]);
        }

        for l in 1..=input.ndor {
            chg[l - 1] = aprdev(input.bg_i, input.bp_j, l);
        }
        b = input.fl_i + input.fl_j;

        if input.jnd < 0 {
            ensure_length("dg", input.dg.len(), max0)?;
            for (idx, value) in hg.iter_mut().enumerate().take(max0) {
                *value *= input.dg[idx];
            }
            let previous = chg.clone();
            b += input.a;
            for l in 1..=input.ndor {
                chg[l - 1] = aprdev(&previous, input.ag, l);
            }
        }
    } else if abs_jnd == 3 {
        ensure_length("dg", input.dg.len(), 1)?;
        ensure_length("dp", input.dp.len(), 1)?;
        ensure_length("cg_i", input.cg_i.len(), 1)?;
        ensure_length("cp_j", input.cp_j.len(), 1)?;
        ensure_length("bg_i", input.bg_i.len(), input.ndor)?;
        ensure_length("bp_j", input.bp_j.len(), input.ndor)?;

        let max0 = input
            .nmax_i
            .min(input.nmax_j)
            .min(input.dg.len())
            .min(input.dp.len())
            .min(input.cg_i.len())
            .min(input.cp_j.len())
            .min(input.dr.len());
        validate_simpson_points(max0)?;

        hg = Vec::with_capacity(max0);
        for idx in 0..max0 {
            hg.push(input.dg[idx] * input.cg_i[idx] + input.dp[idx] * input.cp_j[idx]);
        }
        b = input.a + input.fl_i;
        for l in 1..=input.ndor {
            chg[l - 1] = aprdev(input.bg_i, input.ag, l) + aprdev(input.bp_j, input.ap, l);
        }
    } else if abs_jnd == 4 {
        ensure_length("dg", input.dg.len(), 1)?;
        ensure_length("dp", input.dp.len(), 1)?;
        let max0 = input
            .integration_limit
            .min(input.dg.len())
            .min(input.dp.len())
            .min(input.dr.len());
        validate_simpson_points(max0)?;

        hg = Vec::with_capacity(max0);
        for idx in 0..max0 {
            hg.push(input.dg[idx] * input.dg[idx] + input.dp[idx] * input.dp[idx]);
        }

        b = input.a + input.a;
        for l in 1..=input.ndor {
            chg[l - 1] = aprdev(input.ag, input.ag, l) + aprdev(input.ap, input.ap, l);
        }
    } else {
        let prebuilt_hg = input.prebuilt_hg.ok_or(DsordfError::MissingPrebuiltHg)?;
        let max0 = input
            .integration_limit
            .min(prebuilt_hg.len())
            .min(input.dr.len());
        validate_simpson_points(max0)?;

        hg = prebuilt_hg[..max0].to_vec();
        if let Some(prebuilt_chg) = input.prebuilt_chg {
            ensure_length("prebuilt_chg", prebuilt_chg.len(), input.ndor)?;
            chg.copy_from_slice(&prebuilt_chg[..input.ndor]);
        }
        b = input.a;
    }

    let max0 = hg.len();
    let io = input.n + 1;
    for (idx, value) in hg.iter_mut().enumerate().take(max0) {
        *value *= input.dr[idx].powi(io);
    }

    let mut value = 0.0;
    let mut idx = 1;
    while idx < max0 - 1 {
        value += hg[idx] + hg[idx] + hg[idx + 1];
        idx += 2;
    }
    value = input.hx * (value + value + hg[0] - hg[max0 - 1]) / 3.0;

    b += input.n as f64;
    for (term_index, chg_value) in chg.iter().enumerate() {
        b += 1.0;
        if b == 0.0 {
            return Err(DsordfError::SingularOriginTerm {
                term_index: term_index + 1,
            });
        }
        value += chg_value * input.dr[0].powf(b) / b;
    }

    Ok(value)
}

fn ensure_length(name: &'static str, got: usize, need: usize) -> Result<(), DsordfError> {
    if got < need {
        return Err(DsordfError::LengthMismatch { name, need, got });
    }
    Ok(())
}

fn validate_simpson_points(max0: usize) -> Result<(), DsordfError> {
    if max0 < 3 || max0.is_multiple_of(2) {
        return Err(DsordfError::InvalidRadialPointCount(max0));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DsordfError, DsordfInput, dsordf};

    fn base_input(jnd: i32) -> DsordfInput<'static> {
        DsordfInput {
            n: 0,
            jnd,
            a: 0.0,
            ndor: 1,
            hx: 1.0,
            dr: &[1.0, 2.0, 3.0, 4.0, 5.0],
            dg: &[2.0, 2.0, 2.0, 2.0, 2.0],
            dp: &[0.0, 0.0, 0.0, 0.0, 0.0],
            ag: &[1.0],
            ap: &[0.0],
            cg_i: &[1.0, 1.0, 1.0, 1.0, 1.0],
            cg_j: &[1.0, 1.0, 1.0, 1.0, 1.0],
            cp_i: &[0.0, 0.0, 0.0, 0.0, 0.0],
            cp_j: &[0.0, 0.0, 0.0, 0.0, 0.0],
            bg_i: &[1.0],
            bg_j: &[2.0],
            bp_i: &[0.0],
            bp_j: &[0.0],
            fl_i: 0.0,
            fl_j: 0.0,
            nmax_i: 5,
            nmax_j: 5,
            integration_limit: 5,
            prebuilt_hg: None,
            prebuilt_chg: None,
        }
    }

    #[test]
    fn dsordf_matches_reference_for_jnd_one() {
        let input = base_input(1);
        let value = dsordf(&input).expect("valid dsordf call should succeed");
        assert!((value - 14.0).abs() <= 1.0e-12);
    }

    #[test]
    fn dsordf_applies_negative_jnd_weighting() {
        let input = base_input(-1);
        let value = dsordf(&input).expect("negative jnd branch should succeed");
        assert!((value - 26.0).abs() <= 1.0e-12);
    }

    #[test]
    fn dsordf_handles_jnd_four_density_norm_case() {
        let mut input = base_input(4);
        input.dg = &[1.0, 1.0, 1.0, 1.0, 1.0];
        let value = dsordf(&input).expect("jnd=4 should succeed");
        assert!((value - 13.0).abs() <= 1.0e-12);
    }

    #[test]
    fn dsordf_uses_prebuilt_arrays_for_high_modes() {
        let mut input = base_input(6);
        input.prebuilt_hg = Some(&[1.0, 1.0, 1.0, 1.0, 1.0]);
        input.prebuilt_chg = Some(&[3.0]);
        let value = dsordf(&input).expect("jnd>=5 should accept prebuilt arrays");
        assert!((value - 15.0).abs() <= 1.0e-12);
    }

    #[test]
    fn dsordf_rejects_even_grid_for_simpson() {
        let mut input = base_input(1);
        input.dr = &[1.0, 2.0, 3.0, 4.0];
        input.dg = &[1.0, 1.0, 1.0, 1.0];
        input.dp = &[0.0, 0.0, 0.0, 0.0];
        input.cg_i = &[1.0, 1.0, 1.0, 1.0];
        input.cg_j = &[1.0, 1.0, 1.0, 1.0];
        input.cp_i = &[0.0, 0.0, 0.0, 0.0];
        input.cp_j = &[0.0, 0.0, 0.0, 0.0];
        input.nmax_i = 4;
        input.nmax_j = 4;

        let error = dsordf(&input).expect_err("even grid should fail");
        assert_eq!(error, DsordfError::InvalidRadialPointCount(4));
    }
}
