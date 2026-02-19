#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum XstarError {
    #[error("lfin={0} is unsupported; valid range is 1..=4")]
    UnsupportedLfin(usize),
    #[error("vector {0} has zero norm")]
    ZeroNormVector(&'static str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct XstarInput {
    pub eps1: [f64; 3],
    pub eps2: [f64; 3],
    pub vec1: [f64; 3],
    pub vec2: [f64; 3],
    pub ndeg: f64,
    pub elpty: f64,
    pub lfin: usize,
}

pub fn xstar(input: &XstarInput) -> Result<f64, XstarError> {
    let x = xxcos(input.vec1, input.vec2, "vec1/vec2")?;

    let y1 = xxcos(input.eps1, input.vec1, "eps1")?;
    let z1 = xxcos(input.eps1, input.vec2, "eps1")?;
    let mut xtemp = ystar(input.lfin, x, y1, z1, true)?;

    if input.elpty.abs() > f64::EPSILON {
        let y2 = xxcos(input.eps2, input.vec1, "eps2")?;
        let z2 = xxcos(input.eps2, input.vec2, "eps2")?;
        xtemp += input.elpty * input.elpty * ystar(input.lfin, x, y2, z2, true)?;
    }

    Ok(input.ndeg * xtemp / (1.0 + input.elpty * input.elpty))
}

fn xxcos(vec_a: [f64; 3], vec_b: [f64; 3], name: &'static str) -> Result<f64, XstarError> {
    let dot = vec_a
        .iter()
        .zip(vec_b.iter())
        .map(|(lhs, rhs)| lhs * rhs)
        .sum::<f64>();

    let norm_a = vec_a.iter().map(|value| value * value).sum::<f64>().sqrt();
    let norm_b = vec_b.iter().map(|value| value * value).sum::<f64>().sqrt();

    if norm_a <= f64::EPSILON || norm_b <= f64::EPSILON {
        return Err(XstarError::ZeroNormVector(name));
    }

    Ok(dot / (norm_a * norm_b))
}

fn ystar(lfin: usize, x: f64, y: f64, z: f64, iav: bool) -> Result<f64, XstarError> {
    let coeffs = legendre_coeffs(lfin)?;

    let mut pln0 = coeffs[0];
    for (power, coeff) in coeffs.iter().enumerate().skip(1).take(lfin) {
        pln0 += coeff * x.powi(power as i32);
    }

    if !iav {
        return Ok(pln0 / (2 * lfin + 1) as f64);
    }

    let mut pln1 = coeffs[1];
    for (power, coeff) in coeffs
        .iter()
        .enumerate()
        .skip(2)
        .take(lfin.saturating_sub(1))
    {
        pln1 += coeff * power as f64 * x.powi(power as i32 - 1);
    }

    let mut pln2 = 2.0 * coeffs[2];
    for (power, coeff) in coeffs
        .iter()
        .enumerate()
        .skip(3)
        .take(lfin.saturating_sub(2))
    {
        pln2 += coeff * power as f64 * (power as f64 - 1.0) * x.powi(power as i32 - 2);
    }

    let l = lfin as f64;
    let ytemp = -l * pln0 + pln1 * (x + y * z) - pln2 * (y * y + z * z - 2.0 * x * y * z);
    Ok(ytemp * 3.0 / l / (4.0 * l * l - 1.0))
}

fn legendre_coeffs(lfin: usize) -> Result<[f64; 5], XstarError> {
    match lfin {
        1 => Ok([0.0, 1.0, 0.0, 0.0, 0.0]),
        2 => Ok([-0.5, 0.0, 1.5, 0.0, 0.0]),
        3 => Ok([0.0, -1.5, 0.0, 2.5, 0.0]),
        4 => Ok([0.375, 0.0, -3.75, 0.0, 4.375]),
        _ => Err(XstarError::UnsupportedLfin(lfin)),
    }
}

#[cfg(test)]
mod tests {
    use super::{XstarError, XstarInput, xstar};

    #[test]
    fn xstar_matches_simple_collinear_case() {
        let result = xstar(&XstarInput {
            eps1: [1.0, 0.0, 0.0],
            eps2: [1.0, 0.0, 0.0],
            vec1: [1.0, 0.0, 0.0],
            vec2: [1.0, 0.0, 0.0],
            ndeg: 2.0,
            elpty: 0.0,
            lfin: 1,
        })
        .expect("valid vectors should compute xstar");

        assert!((result - 2.0).abs() < 1.0e-12);
    }

    #[test]
    fn xstar_includes_ellipticity_averaging() {
        let linear = xstar(&XstarInput {
            eps1: [1.0, 0.0, 0.0],
            eps2: [0.0, 0.0, 1.0],
            vec1: [1.0, 0.0, 0.0],
            vec2: [1.0, 1.0, 0.0],
            ndeg: 1.0,
            elpty: 0.0,
            lfin: 3,
        })
        .expect("linear case should compute");

        let elliptical = xstar(&XstarInput {
            eps1: [1.0, 0.0, 0.0],
            eps2: [0.0, 0.0, 1.0],
            vec1: [1.0, 0.0, 0.0],
            vec2: [1.0, 1.0, 0.0],
            ndeg: 1.0,
            elpty: 0.75,
            lfin: 3,
        })
        .expect("elliptical case should compute");

        assert_ne!(linear, elliptical);
    }

    #[test]
    fn xstar_rejects_zero_length_vectors() {
        let error = xstar(&XstarInput {
            eps1: [0.0, 0.0, 0.0],
            eps2: [1.0, 0.0, 0.0],
            vec1: [1.0, 0.0, 0.0],
            vec2: [1.0, 0.0, 0.0],
            ndeg: 1.0,
            elpty: 0.0,
            lfin: 1,
        })
        .expect_err("zero-length vectors should fail");

        assert_eq!(error, XstarError::ZeroNormVector("eps1"));
    }
}
