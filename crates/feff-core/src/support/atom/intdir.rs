const NPI: usize = 5;
const HXD: f64 = 720.0;
const CMIXN: f64 = 473.0;
const CMIXD: f64 = 502.0;

#[derive(Debug, Clone)]
pub struct IntdirInput<'a> {
    pub en: f64,
    pub fl: f64,
    pub agi: f64,
    pub api: f64,
    pub ainf: f64,
    pub max0: usize,
    pub mat: usize,
    pub imm: i32,
    pub ell: f64,
    pub fk: f64,
    pub ccl: f64,
    pub cl: f64,
    pub hx: f64,
    pub test1: f64,
    pub ndor: usize,
    pub np: usize,
    pub dr: &'a [f64],
    pub dv: &'a [f64],
    pub av: &'a [f64],
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntdirOutput {
    pub gg: Vec<f64>,
    pub gp: Vec<f64>,
    pub ag: Vec<f64>,
    pub ap: Vec<f64>,
    pub ggmat: f64,
    pub gpmat: f64,
    pub mat: usize,
    pub max0: usize,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum IntdirError {
    #[error("invalid dimensions (np={np}, ndor={ndor}, dr={dr}, dv={dv}, av={av})")]
    InvalidDimensions {
        np: usize,
        ndor: usize,
        dr: usize,
        dv: usize,
        av: usize,
    },
    #[error("cl must be non-zero")]
    ZeroCl,
    #[error("matching point not found")]
    MatchingPointNotFound,
    #[error("last tabulation point is too close to matching point")]
    TailTooCloseToMatch,
    #[error("integration stepped out of radial grid")]
    IntegrationOutOfBounds,
    #[error("inward asymptotic factor requires -ec*(ccl+ec) >= 0, got {0}")]
    InvalidAsymptoticDomain(f64),
}

pub fn intdir(input: &IntdirInput<'_>) -> Result<IntdirOutput, IntdirError> {
    if input.cl == 0.0 {
        return Err(IntdirError::ZeroCl);
    }
    if input.np < NPI + 2
        || input.ndor < 1
        || input.dr.len() < input.np
        || input.dv.len() < input.np
        || input.av.len() < input.ndor
    {
        return Err(IntdirError::InvalidDimensions {
            np: input.np,
            ndor: input.ndor,
            dr: input.dr.len(),
            dv: input.dv.len(),
            av: input.av.len(),
        });
    }

    let (cop, coc, cmc) = transformed_pc_coefficients();
    let mut c = input.hx / HXD;
    let ec = input.en / input.cl;

    let mut gg = vec![0.0_f64; input.np];
    let mut gp = vec![0.0_f64; input.np];
    let mut dg = vec![0.0_f64; NPI];
    let mut dp = vec![0.0_f64; NPI];

    let mut ag = vec![0.0_f64; input.ndor];
    let mut ap = vec![0.0_f64; input.ndor];
    ag[0] = input.agi;
    ap[0] = input.api;

    let mut mat = input.mat;
    let mut max0 = input.max0;
    let mut ainf = input.ainf;
    let mut ggmat = 0.0_f64;
    let mut gpmat = 0.0_f64;

    if input.imm == 0 {
        mat = find_matching_point(input.np, input.dr, input.dv, input.ell, ec)?;
    } else {
        if mat == 0 || mat > input.np {
            return Err(IntdirError::IntegrationOutOfBounds);
        }
        if max0 == 0 || max0 > input.np {
            max0 = input.np;
        }
    }

    if input.imm >= 0 {
        initialize_series(
            input.ndor, input.fl, input.fk, input.ccl, ec, c, input.av, input.dr, &mut ag, &mut ap,
            &mut gg, &mut gp, &mut dg, &mut dp,
        );

        let mut i = NPI;
        let mut k = 1_i32;
        ggmat = gg[mat - 1];
        gpmat = gp[mat - 1];

        integrate_to_match(
            &mut i, &mut k, mat, c, cmc, &cop, &coc, ec, input.fk, input.ccl, input.dr, input.dv,
            &mut gg, &mut gp, &mut dg, &mut dp,
        )?;

        std::mem::swap(&mut ggmat, &mut gg[mat - 1]);
        std::mem::swap(&mut gpmat, &mut gp[mat - 1]);

        if input.imm == 0 {
            let threshold = input.test1 * ggmat.abs();
            if ainf > threshold {
                ainf = threshold;
            }
            max0 = locate_tail_start(input.np, mat, input.cl, ec, input.dr, input.dv)?;
        }
    }

    c = -c;
    let mut a_sq = -ec * (input.ccl + ec);
    if a_sq < 0.0 {
        return Err(IntdirError::InvalidAsymptoticDomain(a_sq));
    }

    let mut a = -a_sq.sqrt();
    while a * input.dr[max0 - 1] < -170.0 {
        max0 = locate_tail_start(max0, mat, input.cl, ec, input.dr, input.dv)?;
        a_sq = -ec * (input.ccl + ec);
        if a_sq < 0.0 {
            return Err(IntdirError::InvalidAsymptoticDomain(a_sq));
        }
        a = -a_sq.sqrt();
    }

    let b = a / (input.ccl + ec);
    let mut f = ainf / (a * input.dr[max0 - 1]).exp();
    if f == 0.0 {
        f = 1.0;
    }

    let mut idx = 1usize;
    while idx <= NPI {
        let j = max0 + 1 - idx;
        gg[j - 1] = f * (a * input.dr[j - 1]).exp();
        gp[j - 1] = b * gg[j - 1];
        dg[idx - 1] = a * input.dr[j - 1] * gg[j - 1] * c;
        dp[idx - 1] = b * dg[idx - 1];
        idx += 1;
    }

    let mut i = max0 - NPI + 1;
    let mut k = -1_i32;
    integrate_to_match(
        &mut i, &mut k, mat, c, cmc, &cop, &coc, ec, input.fk, input.ccl, input.dr, input.dv,
        &mut gg, &mut gp, &mut dg, &mut dp,
    )?;

    if ggmat == 0.0 {
        ggmat = gg[mat - 1];
        gpmat = gp[mat - 1];
    }

    Ok(IntdirOutput {
        gg,
        gp,
        ag,
        ap,
        ggmat,
        gpmat,
        mat,
        max0,
    })
}

fn transformed_pc_coefficients() -> ([f64; NPI], [f64; NPI], f64) {
    let cop = [251.0, -1274.0, 2616.0, -2774.0, 1901.0];
    let mut coc = [-19.0, 106.0, -264.0, 646.0, 251.0];

    let c_mix = CMIXN / CMIXD;
    let a_mix = 1.0 - c_mix;
    let cmc = c_mix * coc[NPI - 1];

    let mut f = coc[0];
    let mut j = 1usize;
    while j < NPI {
        let g = coc[j];
        coc[j] = c_mix * f + a_mix * cop[j];
        f = g;
        j += 1;
    }
    coc[0] = c_mix * cop[0];

    (cop, coc, cmc)
}

fn find_matching_point(
    np: usize,
    dr: &[f64],
    dv: &[f64],
    ell: f64,
    ec: f64,
) -> Result<usize, IntdirError> {
    let mut mat = NPI;
    let mut j = 1_i32;

    loop {
        mat += 2;
        if mat >= np {
            if ec > -0.0003 {
                return Ok(np - 12);
            }
            return Err(IntdirError::MatchingPointNotFound);
        }

        let mut f = dv[mat - 1] + ell / (dr[mat - 1] * dr[mat - 1]);
        f *= j as f64;
        f -= ec * j as f64;

        if f <= 0.0 {
            j = -j;
            if j < 0 {
                continue;
            }
            if mat >= np - NPI {
                mat = np - 12;
            }
            return Ok(mat);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn initialize_series(
    ndor: usize,
    fl: f64,
    fk: f64,
    ccl: f64,
    ec: f64,
    c: f64,
    av: &[f64],
    dr: &[f64],
    ag: &mut [f64],
    ap: &mut [f64],
    gg: &mut [f64],
    gp: &mut [f64],
    dg: &mut [f64],
    dp: &mut [f64],
) {
    let mut j = 2usize;
    while j <= ndor {
        let k = j - 1;
        let a = fl + fk + k as f64;
        let b = fl - fk + k as f64;
        let ep = a * b + av[0] * av[0];

        let mut f = (ec + ccl) * ap[k - 1] + ap[j - 1];
        let mut g = ec * ag[k - 1] + ag[j - 1];

        let mut i = 1usize;
        while i <= k {
            f -= av[i] * ap[j - 1 - i];
            g -= av[i] * ag[j - 1 - i];
            i += 1;
        }

        ag[j - 1] = (b * f + av[0] * g) / ep;
        ap[j - 1] = (av[0] * f - a * g) / ep;
        j += 1;
    }

    let mut i = 1usize;
    while i <= NPI {
        gg[i - 1] = 0.0;
        gp[i - 1] = 0.0;
        dg[i - 1] = 0.0;
        dp[i - 1] = 0.0;

        let mut j = 1usize;
        while j <= ndor {
            let p = fl + (j - 1) as f64;
            let r_pow = dr[i - 1].powf(p);
            let deriv = p * r_pow * c;
            gg[i - 1] += r_pow * ag[j - 1];
            gp[i - 1] += r_pow * ap[j - 1];
            dg[i - 1] += deriv * ag[j - 1];
            dp[i - 1] += deriv * ap[j - 1];
            j += 1;
        }

        i += 1;
    }
}

#[allow(clippy::too_many_arguments)]
fn integrate_to_match(
    i: &mut usize,
    k: &mut i32,
    mat: usize,
    c: f64,
    cmc: f64,
    cop: &[f64; NPI],
    coc: &[f64; NPI],
    ec: f64,
    fk: f64,
    ccl: f64,
    dr: &[f64],
    dv: &[f64],
    gg: &mut [f64],
    gp: &mut [f64],
    dg: &mut [f64],
    dp: &mut [f64],
) -> Result<(), IntdirError> {
    let cmcc = cmc * c;

    loop {
        let mut a = gg[*i - 1] + dg[0] * cop[0];
        let mut b = gp[*i - 1] + dp[0] * cop[0];

        if *k > 0 {
            *i += 1;
        } else {
            if *i <= 1 {
                return Err(IntdirError::IntegrationOutOfBounds);
            }
            *i -= 1;
        }

        if *i == 0 || *i > gg.len() {
            return Err(IntdirError::IntegrationOutOfBounds);
        }

        let ep = gp[*i - 1];
        let eg = gg[*i - 1];
        gg[*i - 1] = a - dg[0] * coc[0];
        gp[*i - 1] = b - dp[0] * coc[0];

        let mut j = 1usize;
        while j < NPI {
            a += dg[j] * cop[j];
            b += dp[j] * cop[j];
            gg[*i - 1] += dg[j] * coc[j];
            gp[*i - 1] += dp[j] * coc[j];
            dg[j - 1] = dg[j];
            dp[j - 1] = dp[j];
            j += 1;
        }

        let f = (ec - dv[*i - 1]) * dr[*i - 1];
        let g = f + ccl * dr[*i - 1];
        gg[*i - 1] += cmcc * (g * b - fk * a + ep);
        gp[*i - 1] += cmcc * (fk * b - f * a - eg);

        dg[NPI - 1] = c * (g * gp[*i - 1] - fk * gg[*i - 1] + ep);
        dp[NPI - 1] = c * (fk * gp[*i - 1] - f * gg[*i - 1] - eg);

        if *i == mat {
            break;
        }
    }

    Ok(())
}

fn locate_tail_start(
    mut max0: usize,
    mat: usize,
    cl: f64,
    ec: f64,
    dr: &[f64],
    dv: &[f64],
) -> Result<usize, IntdirError> {
    let a_limit = 700.0 / cl;

    loop {
        if max0 <= 2 {
            return Err(IntdirError::TailTooCloseToMatch);
        }
        max0 -= 2;

        if max0 < mat + NPI {
            return Err(IntdirError::TailTooCloseToMatch);
        }

        let criterion = (dv[max0 - 1] - ec) * dr[max0 - 1] * dr[max0 - 1];
        if criterion <= a_limit {
            return Ok(max0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{IntdirError, IntdirInput, intdir};

    #[test]
    fn intdir_computes_finite_outward_and_inward_solutions() {
        let np = 25usize;
        let ndor = 5usize;

        let mut dr = vec![0.0_f64; np];
        let mut dv = vec![0.0_f64; np];
        let mut i = 0usize;
        while i < np {
            dr[i] = 1.0 + i as f64;
            dv[i] = if i < 12 { -1.0 } else { 1.0 };
            i += 1;
        }

        let output = intdir(&IntdirInput {
            en: -0.5,
            fl: 0.5,
            agi: 1.0,
            api: 0.1,
            ainf: 1.0e-5,
            max0: np,
            mat: 13,
            imm: 0,
            ell: 0.0,
            fk: 0.0,
            ccl: 1.0,
            cl: 1.0,
            hx: 0.05,
            test1: 1.0e-4,
            ndor,
            np,
            dr: &dr,
            dv: &dv,
            av: &[1.0, 0.0, 0.0, 0.0, 0.0],
        })
        .expect("intdir should succeed");

        assert_eq!(output.gg.len(), np);
        assert_eq!(output.gp.len(), np);
        assert!(output.gg.iter().all(|value| value.is_finite()));
        assert!(output.gp.iter().all(|value| value.is_finite()));
        assert!(output.mat > 0 && output.mat <= np);
    }

    #[test]
    fn intdir_reports_matching_point_failures() {
        let np = 25usize;
        let ndor = 5usize;
        let dr = vec![1.0_f64; np];
        let dv = vec![1.0_f64; np];

        let error = intdir(&IntdirInput {
            en: -1.0,
            fl: 0.5,
            agi: 1.0,
            api: 0.0,
            ainf: 1.0e-6,
            max0: np,
            mat: 13,
            imm: 0,
            ell: 0.0,
            fk: 0.0,
            ccl: 1.0,
            cl: 1.0,
            hx: 0.05,
            test1: 1.0e-4,
            ndor,
            np,
            dr: &dr,
            dv: &dv,
            av: &[1.0, 0.0, 0.0, 0.0, 0.0],
        })
        .expect_err("intdir should fail when turning points are absent");

        assert_eq!(error, IntdirError::MatchingPointNotFound);
    }

    #[test]
    fn intdir_validates_input_dimensions() {
        let error = intdir(&IntdirInput {
            en: -1.0,
            fl: 0.5,
            agi: 1.0,
            api: 0.0,
            ainf: 1.0e-6,
            max0: 3,
            mat: 1,
            imm: 1,
            ell: 0.0,
            fk: 0.0,
            ccl: 1.0,
            cl: 1.0,
            hx: 0.05,
            test1: 1.0e-4,
            ndor: 0,
            np: 3,
            dr: &[1.0, 2.0, 3.0],
            dv: &[1.0, 2.0, 3.0],
            av: &[],
        })
        .expect_err("invalid dimensions must fail");

        assert!(matches!(error, IntdirError::InvalidDimensions { .. }));
    }
}
