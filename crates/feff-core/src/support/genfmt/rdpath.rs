use num_complex::Complex64;

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum RdpathError {
    #[error("missing path header line")]
    MissingHeader,
    #[error("path header must include ipath, nleg, and degeneracy")]
    InvalidHeader,
    #[error("path label line is missing")]
    MissingLabelLine,
    #[error("nleg must be positive")]
    InvalidNleg,
    #[error("missing atom line for leg index {0}")]
    MissingLegLine(usize),
    #[error("invalid atom line for leg index {0}")]
    InvalidLegLine(usize),
    #[error("ipot={ipot} at leg={leg} exceeds npot={npot}")]
    InvalidPotential {
        ipot: usize,
        leg: usize,
        npot: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathLeg {
    pub position: [f64; 3],
    pub ipot: usize,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedPath {
    pub ipath: i32,
    pub nleg: usize,
    pub degeneracy: f64,
    pub legs: Vec<PathLeg>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathAngles {
    pub nsc: usize,
    pub beta: Vec<f64>,
    pub eta: Vec<f64>,
    pub eta_polarization: Option<(f64, f64)>,
    pub ri: Vec<f64>,
    pub alpha: Vec<f64>,
    pub gamma: Vec<f64>,
}

pub fn rdpath(
    input: &str,
    ipol: bool,
    bohr: f64,
    npot: usize,
) -> Result<(ParsedPath, PathAngles), RdpathError> {
    let mut lines = input.lines().map(str::trim).filter(|line| !line.is_empty());

    let header_line = lines.next().ok_or(RdpathError::MissingHeader)?;
    let mut header_tokens = header_line.split_whitespace();
    let ipath = header_tokens
        .next()
        .and_then(|token| token.parse::<i32>().ok())
        .ok_or(RdpathError::InvalidHeader)?;
    let nleg = header_tokens
        .next()
        .and_then(|token| token.parse::<usize>().ok())
        .ok_or(RdpathError::InvalidHeader)?;
    let degeneracy = header_tokens
        .next()
        .and_then(|token| token.parse::<f64>().ok())
        .ok_or(RdpathError::InvalidHeader)?;

    if nleg == 0 {
        return Err(RdpathError::InvalidNleg);
    }

    lines.next().ok_or(RdpathError::MissingLabelLine)?;

    let mut legs = Vec::with_capacity(nleg);
    for leg_index in 1..=nleg {
        let line = lines.next().ok_or(RdpathError::MissingLegLine(leg_index))?;
        let leg = parse_leg_line(line, leg_index, bohr, npot)?;
        legs.push(leg);
    }

    let parsed = ParsedPath {
        ipath,
        nleg,
        degeneracy,
        legs,
    };

    let angles = compute_angles(&parsed.legs, ipol);
    Ok((parsed, angles))
}

fn parse_leg_line(
    line: &str,
    leg_index: usize,
    bohr: f64,
    npot: usize,
) -> Result<PathLeg, RdpathError> {
    let mut tokens = line.split_whitespace();
    let x = tokens
        .next()
        .and_then(|token| token.parse::<f64>().ok())
        .ok_or(RdpathError::InvalidLegLine(leg_index))?;
    let y = tokens
        .next()
        .and_then(|token| token.parse::<f64>().ok())
        .ok_or(RdpathError::InvalidLegLine(leg_index))?;
    let z = tokens
        .next()
        .and_then(|token| token.parse::<f64>().ok())
        .ok_or(RdpathError::InvalidLegLine(leg_index))?;
    let ipot = tokens
        .next()
        .and_then(|token| token.parse::<usize>().ok())
        .ok_or(RdpathError::InvalidLegLine(leg_index))?;
    let label = tokens.next().unwrap_or("").to_string();

    if ipot > npot {
        return Err(RdpathError::InvalidPotential {
            ipot,
            leg: leg_index,
            npot,
        });
    }

    Ok(PathLeg {
        position: [x / bohr, y / bohr, z / bohr],
        ipot,
        label,
    })
}

fn compute_angles(legs: &[PathLeg], ipol: bool) -> PathAngles {
    let nleg = legs.len();
    let nsc = nleg.saturating_sub(1);
    let nangle = nleg + usize::from(ipol);

    let mut rat = vec![[0.0_f64; 3]; nleg + 2];
    for (index, leg) in legs.iter().enumerate() {
        rat[index + 1] = leg.position;
    }

    if ipol {
        let tail = rat[nleg];
        rat[nleg + 1] = [tail[0], tail[1], tail[2] + 1.0];
    }
    rat[0] = rat[nleg];

    let mut alpha = vec![0.0_f64; nangle + 1];
    let mut gamma = vec![0.0_f64; nangle + 1];
    let mut beta = vec![0.0_f64; nangle + 1];
    let mut ri = vec![0.0_f64; nleg];

    for j in 1..=nangle {
        let (i, ip1, im1, fix_reference) = if j == nsc + 1 {
            let ip1 = if ipol { nleg + 1 } else { 1 };
            (0, ip1, nsc, false)
        } else if j == nsc + 2 {
            (0, 1, nleg + 1, true)
        } else {
            (j, j + 1, j - 1, false)
        };

        let forward = sub(rat[ip1], rat[i]);
        let mut backward = sub(rat[i], rat[im1]);
        let (ctp, stp, cpp, spp) = trig(forward);

        if fix_reference {
            backward = [0.0, 0.0, 1.0];
        }
        let (ct, st, cp, sp) = trig(backward);

        let cppp = cp * cpp + sp * spp;
        let sppp = spp * cp - cpp * sp;
        let phi = sp.atan2(cp);
        let phip = spp.atan2(cpp);

        let alph = -(st * ctp - ct * stp * cppp - Complex64::new(0.0, 1.0) * stp * sppp);
        let mut beta_value = ct * ctp + st * stp * cppp;
        beta_value = beta_value.clamp(-1.0, 1.0);
        let gamm = -(st * ctp * cppp - ct * stp + Complex64::new(0.0, 1.0) * st * sppp);

        let mut alpha_j = arg(alph, phip - phi);
        let mut gamma_j = arg(gamm, 0.0);

        let saved_alpha = alpha_j;
        alpha_j = std::f64::consts::PI - gamma_j;
        gamma_j = std::f64::consts::PI - saved_alpha;

        alpha[j] = alpha_j;
        gamma[j] = gamma_j;
        beta[j] = beta_value.acos();

        if j <= nleg {
            ri[j - 1] = dist(rat[i], rat[im1]);
        }
    }

    alpha[0] = alpha[nangle];

    let mut eta = vec![0.0_f64; nleg];
    for j in 1..=nleg {
        eta[j - 1] = alpha[j - 1] + gamma[j];
    }

    let eta_polarization = if ipol {
        Some((gamma[nleg + 1], alpha[nleg]))
    } else {
        None
    };

    PathAngles {
        nsc,
        beta: beta[1..=nangle].to_vec(),
        eta,
        eta_polarization,
        ri,
        alpha: alpha[1..=nangle].to_vec(),
        gamma: gamma[1..=nangle].to_vec(),
    }
}

fn trig(vector: [f64; 3]) -> (f64, f64, f64, f64) {
    let [x, y, z] = vector;
    let eps = 1.0e-6;

    let r = (x * x + y * y + z * z).sqrt();
    let rxy = (x * x + y * y).sqrt();

    let (ct, st) = if r < eps {
        (1.0, 0.0)
    } else {
        (z / r, rxy / r)
    };

    let (cp, sp) = if rxy < eps {
        (if ct < 0.0 { -1.0 } else { 1.0 }, 0.0)
    } else {
        (x / rxy, y / rxy)
    };

    (ct, st, cp, sp)
}

fn arg(value: Complex64, fallback: f64) -> f64 {
    let eps = 1.0e-6;
    let mut x = value.re;
    let mut y = value.im;
    if x.abs() < eps {
        x = 0.0;
    }
    if y.abs() < eps {
        y = 0.0;
    }

    if x.abs() < eps && y.abs() < eps {
        fallback
    } else {
        y.atan2(x)
    }
}

fn dist(lhs: [f64; 3], rhs: [f64; 3]) -> f64 {
    lhs.iter()
        .zip(rhs.iter())
        .map(|(left, right)| {
            let delta = left - right;
            delta * delta
        })
        .sum::<f64>()
        .sqrt()
}

fn sub(lhs: [f64; 3], rhs: [f64; 3]) -> [f64; 3] {
    [lhs[0] - rhs[0], lhs[1] - rhs[1], lhs[2] - rhs[2]]
}

#[cfg(test)]
mod tests {
    use super::{RdpathError, rdpath};

    fn sample_path() -> &'static str {
        "1 3 2.0
skip
0.0 0.0 0.0 1 p0
1.0 0.0 0.0 1 p0
1.0 1.0 0.0 1 p0"
    }

    #[test]
    fn rdpath_parses_path_and_computes_angles() {
        let (path, angles) = rdpath(sample_path(), false, 1.0, 3).expect("path should parse");

        assert_eq!(path.ipath, 1);
        assert_eq!(path.nleg, 3);
        assert_eq!(angles.nsc, 2);
        assert_eq!(angles.beta.len(), 3);
        assert_eq!(angles.eta.len(), 3);
        assert_eq!(angles.ri.len(), 3);
        assert!(angles.ri[1] > 0.0);
    }

    #[test]
    fn rdpath_adds_polarization_rotation_when_ipol_enabled() {
        let (_, angles) = rdpath(sample_path(), true, 1.0, 3).expect("path should parse");

        assert_eq!(angles.beta.len(), 4);
        assert!(angles.eta_polarization.is_some());
    }

    #[test]
    fn rdpath_rejects_out_of_range_ipot() {
        let error = rdpath(sample_path(), false, 1.0, 0).expect_err("ipot > npot should fail");
        assert_eq!(
            error,
            RdpathError::InvalidPotential {
                ipot: 1,
                leg: 1,
                npot: 0,
            }
        );
    }
}
