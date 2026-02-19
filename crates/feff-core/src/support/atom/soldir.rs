use super::intdir::{IntdirError, IntdirInput, IntdirOutput, intdir};

#[derive(Debug, Clone)]
pub struct SoldirInput<'a> {
    pub en: f64,
    pub fl: f64,
    pub agi: f64,
    pub api: f64,
    pub ainf: f64,
    pub nq: i32,
    pub kap: i32,
    pub max0: usize,
    pub method: i32,
    pub cl: f64,
    pub dv: &'a [f64],
    pub av: &'a [f64],
    pub dr: &'a [f64],
    pub hx: f64,
    pub test1: f64,
    pub test2: f64,
    pub ndor: usize,
    pub np: usize,
    pub nes: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoldirOutput {
    pub en: f64,
    pub gg: Vec<f64>,
    pub gp: Vec<f64>,
    pub ag: Vec<f64>,
    pub ap: Vec<f64>,
    pub mat: usize,
    pub max0: usize,
    pub method: i32,
    pub ifail: bool,
}

#[derive(Debug, Clone)]
pub struct NormInput<'a> {
    pub gg: &'a [f64],
    pub gp: &'a [f64],
    pub ag: &'a [f64],
    pub ap: &'a [f64],
    pub dr: &'a [f64],
    pub hx: f64,
    pub fl: f64,
    pub max0: usize,
    pub mat: usize,
    pub method: i32,
    pub gpmat: f64,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum SoldirError {
    #[error("cl must be non-zero")]
    ZeroCl,
    #[error("kap must be non-zero")]
    InvalidKappa,
    #[error("np must be >= 7, got {0}")]
    InvalidPointCount(usize),
    #[error("ndor must be >= 1")]
    InvalidNdor,
    #[error("input length mismatch for {name}: need at least {need}, got {got}")]
    LengthMismatch {
        name: &'static str,
        need: usize,
        got: usize,
    },
    #[error("dr[{index}] must be > 0, got {value}")]
    NonPositiveRadius { index: usize, value: f64 },
    #[error("potential minimum is non-negative (emin={0})")]
    NonNegativePotentialMinimum(f64),
    #[error("energy search fell below potential minimum")]
    EnergyBelowPotentialMinimum,
    #[error("failed to bracket target node count within {attempts} attempts")]
    NodeSearchFailed { attempts: usize },
    #[error("wave-function norm must be > 0, got {0}")]
    NonPositiveNorm(f64),
    #[error("series denominator vanished at term {term_index}")]
    SingularNormTerm { term_index: usize },
    #[error(transparent)]
    Intdir(#[from] IntdirError),
}

pub fn soldir(input: &SoldirInput<'_>) -> Result<SoldirOutput, SoldirError> {
    soldir_with(input, |int_input| {
        intdir(int_input).map_err(SoldirError::Intdir)
    })
}

pub fn soldir_with<F>(
    input: &SoldirInput<'_>,
    mut intdir_fn: F,
) -> Result<SoldirOutput, SoldirError>
where
    F: FnMut(&IntdirInput<'_>) -> Result<IntdirOutput, SoldirError>,
{
    validate_input(input)?;

    let mut method = if input.method <= 0 { 1 } else { input.method };
    let test = if method > 1 { input.test2 } else { input.test1 };

    let ccl = input.cl + input.cl;
    let fk = input.kap as f64;
    let ell = fk * (fk + 1.0) / ccl;

    let mut api = input.api;
    if input.av[0] < 0.0 && input.kap > 0 {
        api = -input.agi * (fk + input.fl) / input.av[0];
    }
    if input.av[0] < 0.0 && input.kap < 0 {
        api = -input.agi * input.av[0] / (fk - input.fl);
    }

    let mut node = input.nq - input.kap.abs();
    if input.kap < 0 {
        node += 1;
    }

    let mut emin = 0.0_f64;
    let mut i = 0usize;
    while i < input.np {
        let value = (ell / (input.dr[i] * input.dr[i]) + input.dv[i]) * input.cl;
        if value < emin {
            emin = value;
        }
        i += 1;
    }
    if emin >= 0.0 {
        return Err(SoldirError::NonNegativePotentialMinimum(emin));
    }

    let mut en = input.en;
    if en < emin {
        en = emin * 0.9;
    }

    let mut esup = emin;
    let mut einf = 1.0_f64;
    let mut last = None;

    let mut attempt = 0usize;
    while attempt < input.nes {
        let mat_guess = suggested_match_point(input.np);
        let out = intdir_fn(&IntdirInput {
            en,
            fl: input.fl,
            agi: input.agi,
            api,
            ainf: input.ainf.abs(),
            max0: input.max0.clamp(1, input.np),
            mat: mat_guess,
            imm: 0,
            ell,
            fk,
            ccl,
            cl: input.cl,
            hx: input.hx,
            test1: input.test1,
            ndor: input.ndor,
            np: input.np,
            dr: input.dr,
            dv: input.dv,
            av: input.av,
        })?;

        let mut gg = out.gg;
        let mut gp = out.gp;
        let mut ag = out.ag;
        let mut ap = out.ap;

        let peak = max_amplitude_index(&gg, out.max0.min(input.np));
        let scan_limit = peak.max(out.mat).min(out.max0.max(1));
        let nd = count_nodes(&gg, scan_limit);

        let b = norm(&NormInput {
            gg: &gg,
            gp: &gp,
            ag: &ag,
            ap: &ap,
            dr: input.dr,
            hx: input.hx,
            fl: input.fl,
            max0: out.max0,
            mat: out.mat,
            method,
            gpmat: out.gpmat,
        })?;
        if b <= 0.0 {
            return Err(SoldirError::NonPositiveNorm(b));
        }

        if nd == node {
            normalize_wavefunction(input.agi, api, b, &mut gg, &mut gp, &mut ag, &mut ap);
            return Ok(SoldirOutput {
                en,
                gg,
                gp,
                ag,
                ap,
                mat: out.mat,
                max0: out.max0,
                method,
                ifail: false,
            });
        }

        last = Some((en, out.mat, out.max0, gg, gp, ag, ap, b));

        if nd < node {
            esup = en;
            if einf < 0.0 {
                if (einf - esup).abs() <= input.test1 {
                    break;
                }
                en = (einf + esup) / 2.0;
            } else {
                en *= 0.8;
                if en.abs() <= input.test1 {
                    break;
                }
            }
        } else {
            einf = en;
            if esup > emin {
                if (einf - esup).abs() <= input.test1 {
                    break;
                }
                en = (einf + esup) / 2.0;
            } else {
                en *= 1.2;
                if en <= emin {
                    return Err(SoldirError::EnergyBelowPotentialMinimum);
                }
            }
        }

        if test > 0.0 && en.abs() <= test {
            break;
        }

        attempt += 1;
    }

    if let Some((en_last, mat, max0, mut gg, mut gp, mut ag, mut ap, b)) = last {
        if b <= 0.0 {
            return Err(SoldirError::NonPositiveNorm(b));
        }
        normalize_wavefunction(input.agi, api, b, &mut gg, &mut gp, &mut ag, &mut ap);
        method = method.max(1);
        return Ok(SoldirOutput {
            en: en_last,
            gg,
            gp,
            ag,
            ap,
            mat,
            max0,
            method,
            ifail: true,
        });
    }

    Err(SoldirError::NodeSearchFailed {
        attempts: input.nes,
    })
}

pub fn norm(input: &NormInput<'_>) -> Result<f64, SoldirError> {
    ensure_len("gg", input.gg.len(), input.max0)?;
    ensure_len("gp", input.gp.len(), input.max0)?;
    ensure_len("dr", input.dr.len(), input.max0)?;
    ensure_len("ag", input.ag.len(), 1)?;
    ensure_len("ap", input.ap.len(), 1)?;

    let mut hp = vec![0.0_f64; input.max0];
    let mut i = 0usize;
    while i < input.max0 {
        hp[i] = input.dr[i] * (input.gg[i] * input.gg[i] + input.gp[i] * input.gp[i]);
        i += 1;
    }

    if input.method == 1 && input.mat >= 1 && input.mat <= input.max0 {
        let idx = input.mat - 1;
        hp[idx] +=
            input.dr[idx] * (input.gpmat * input.gpmat - input.gp[idx] * input.gp[idx]) / 2.0;
    }

    let mut b = 0.0_f64;
    let mut j = 1usize;
    while j + 1 < input.max0 {
        b += hp[j] + hp[j] + hp[j + 1];
        j += 2;
    }
    b = input.hx * (b + b + hp[0] - hp[input.max0 - 1]) / 3.0;

    let mut n = 0usize;
    while n < input.ag.len().min(input.ap.len()) {
        let exponent = input.fl + input.fl + (n + 1) as f64;
        if exponent == 0.0 {
            return Err(SoldirError::SingularNormTerm { term_index: n + 1 });
        }
        let g = input.dr[0].powf(exponent) / exponent;

        let mut m = 0usize;
        while m <= n {
            b += input.ag[m] * g * input.ag[n - m] + input.ap[m] * g * input.ap[n - m];
            m += 1;
        }

        n += 1;
    }

    Ok(b)
}

fn normalize_wavefunction(
    agi: f64,
    api: f64,
    b: f64,
    gg: &mut [f64],
    gp: &mut [f64],
    ag: &mut [f64],
    ap: &mut [f64],
) {
    let mut scale = b.sqrt();
    if (ag.first().copied().unwrap_or(0.0) * agi) < 0.0
        || (ap.first().copied().unwrap_or(0.0) * api) < 0.0
    {
        scale = -scale;
    }

    let mut i = 0usize;
    while i < ag.len() {
        ag[i] /= scale;
        i += 1;
    }
    let mut j = 0usize;
    while j < ap.len() {
        ap[j] /= scale;
        j += 1;
    }

    let mut radial_scale = b.sqrt();
    if (gg.first().copied().unwrap_or(0.0) * agi) < 0.0
        || (gp.first().copied().unwrap_or(0.0) * api) < 0.0
    {
        radial_scale = -radial_scale;
    }

    let mut r = 0usize;
    while r < gg.len() {
        gg[r] /= radial_scale;
        r += 1;
    }
    let mut s = 0usize;
    while s < gp.len() {
        gp[s] /= radial_scale;
        s += 1;
    }
}

fn suggested_match_point(np: usize) -> usize {
    if np <= 13 {
        np.saturating_sub(2).max(3)
    } else {
        np - 12
    }
}

fn max_amplitude_index(values: &[f64], max0: usize) -> usize {
    let mut index = 1usize;
    let mut peak = 0.0_f64;

    let mut i = 0usize;
    while i < max0.min(values.len()) {
        let amp = values[i] * values[i];
        if amp > peak {
            peak = amp;
            index = i + 1;
        }
        i += 1;
    }

    index
}

fn count_nodes(gg: &[f64], limit: usize) -> i32 {
    let mut nodes = 1_i32;
    let mut i = 1usize;
    while i < limit.min(gg.len()) {
        if gg[i - 1] != 0.0 && gg[i] / gg[i - 1] <= 0.0 {
            nodes += 1;
        }
        i += 1;
    }
    nodes
}

fn validate_input(input: &SoldirInput<'_>) -> Result<(), SoldirError> {
    if input.cl == 0.0 {
        return Err(SoldirError::ZeroCl);
    }
    if input.kap == 0 {
        return Err(SoldirError::InvalidKappa);
    }
    if input.np < 7 {
        return Err(SoldirError::InvalidPointCount(input.np));
    }
    if input.ndor == 0 {
        return Err(SoldirError::InvalidNdor);
    }

    ensure_len("dv", input.dv.len(), input.np)?;
    ensure_len("av", input.av.len(), input.ndor)?;
    ensure_len("dr", input.dr.len(), input.np)?;

    let mut i = 0usize;
    while i < input.np {
        if input.dr[i] <= 0.0 {
            return Err(SoldirError::NonPositiveRadius {
                index: i,
                value: input.dr[i],
            });
        }
        i += 1;
    }

    Ok(())
}

fn ensure_len(name: &'static str, got: usize, need: usize) -> Result<(), SoldirError> {
    if got < need {
        return Err(SoldirError::LengthMismatch { name, need, got });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{NormInput, SoldirError, SoldirInput, norm, soldir_with};
    use crate::support::atom::intdir::{IntdirInput, IntdirOutput};

    #[test]
    fn norm_matches_simpson_reference_for_flat_wavefunction() {
        let value = norm(&NormInput {
            gg: &[1.0, 1.0, 1.0, 1.0, 1.0],
            gp: &[0.0, 0.0, 0.0, 0.0, 0.0],
            ag: &[0.0],
            ap: &[0.0],
            dr: &[1.0, 1.0, 1.0, 1.0, 1.0],
            hx: 1.0,
            fl: 0.0,
            max0: 5,
            mat: 3,
            method: 0,
            gpmat: 0.0,
        })
        .expect("norm should succeed");

        assert!((value - 4.0).abs() <= 1.0e-12);
    }

    #[test]
    fn soldir_with_mock_intdir_brackets_nodes_and_returns_solution() {
        let input = SoldirInput {
            en: -0.5,
            fl: 1.0,
            agi: 1.0,
            api: 0.2,
            ainf: 1.0,
            nq: 2,
            kap: 1,
            max0: 9,
            method: 1,
            cl: 137.0,
            dv: &[-5.0, -4.0, -3.0, -2.0, -1.5, -1.2, -1.1, -1.05, -1.0],
            av: &[-1.0, 0.0],
            dr: &[0.2, 0.3, 0.4, 0.5, 0.7, 1.0, 1.3, 1.7, 2.1],
            hx: 0.05,
            test1: 1.0e-6,
            test2: 1.0e-6,
            ndor: 2,
            np: 9,
            nes: 20,
        };

        let output = soldir_with(&input, |int_input: &IntdirInput<'_>| {
            let gg = if int_input.en <= -1.0 {
                vec![1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2]
            } else {
                vec![1.0, -0.9, 0.8, -0.7, 0.6, -0.5, 0.4, -0.3, 0.2]
            };

            Ok(IntdirOutput {
                gg,
                gp: vec![0.1; 9],
                ag: vec![0.2, 0.1],
                ap: vec![0.05, 0.02],
                ggmat: 0.7,
                gpmat: 0.1,
                mat: 5,
                max0: 9,
            })
        })
        .expect("mocked soldir should succeed");

        assert!(!output.ifail);
        assert!(output.en <= -1.0);
        assert!(output.gg[0].is_finite());
        assert!(output.ag[0].is_finite());
    }

    #[test]
    fn soldir_rejects_non_negative_potential_minimum() {
        let input = SoldirInput {
            en: -0.1,
            fl: 1.0,
            agi: 1.0,
            api: 0.1,
            ainf: 1.0,
            nq: 1,
            kap: 1,
            max0: 9,
            method: 1,
            cl: 137.0,
            dv: &[1.0; 9],
            av: &[0.0, 0.0],
            dr: &[0.2, 0.3, 0.4, 0.5, 0.7, 1.0, 1.3, 1.7, 2.1],
            hx: 0.05,
            test1: 1.0e-6,
            test2: 1.0e-6,
            ndor: 2,
            np: 9,
            nes: 5,
        };

        let error = soldir_with(&input, |_| {
            unreachable!("intdir should not be called when potential is invalid")
        })
        .expect_err("non-negative potential should fail");

        assert!(matches!(error, SoldirError::NonNegativePotentialMinimum(_)));
    }
}
