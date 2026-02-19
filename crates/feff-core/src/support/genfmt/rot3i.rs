#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum Rot3iError {
    #[error("lxp1 and mxp1 must both be positive")]
    InvalidGrid,
    #[error("angular momentum l={l} exceeds supported limit {limit}")]
    AngularMomentumOutOfRange { l: usize, limit: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rot3iOutput {
    matrices: Vec<Vec<Vec<f64>>>,
    mx_limits: Vec<usize>,
}

impl Rot3iOutput {
    pub fn l_count(&self) -> usize {
        self.matrices.len()
    }

    pub fn matrix_for_l(&self, l: usize) -> Option<&Vec<Vec<f64>>> {
        self.matrices.get(l)
    }

    pub fn get(&self, l: usize, m1: i32, m2: i32) -> Option<f64> {
        let mx = *self.mx_limits.get(l)? as i32;
        if m1.abs() > mx || m2.abs() > mx {
            return None;
        }

        let row = (m1 + mx) as usize;
        let col = (m2 + mx) as usize;
        Some(self.matrices[l][row][col])
    }
}

pub fn rot3i(lxp1: usize, mxp1: usize, beta: f64) -> Result<Rot3iOutput, Rot3iError> {
    if lxp1 == 0 || mxp1 == 0 {
        return Err(Rot3iError::InvalidGrid);
    }

    let lmax = lxp1 - 1;
    const MAX_L_SUPPORTED: usize = 32;
    if lmax > MAX_L_SUPPORTED {
        return Err(Rot3iError::AngularMomentumOutOfRange {
            l: lmax,
            limit: MAX_L_SUPPORTED,
        });
    }

    let mut matrices = Vec::with_capacity(lxp1);
    let mut mx_limits = Vec::with_capacity(lxp1);

    for l in 0..lxp1 {
        let mx = l.min(mxp1 - 1);
        let dim = 2 * mx + 1;
        let mut matrix = vec![vec![0.0; dim]; dim];

        for (row, m1) in (-(mx as i32)..=(mx as i32)).enumerate() {
            for (col, m2) in (-(mx as i32)..=(mx as i32)).enumerate() {
                matrix[row][col] = wigner_small_d(l as i32, m1, m2, beta);
            }
        }

        matrices.push(matrix);
        mx_limits.push(mx);
    }

    Ok(Rot3iOutput {
        matrices,
        mx_limits,
    })
}

fn wigner_small_d(l: i32, m: i32, mp: i32, beta: f64) -> f64 {
    if m.abs() > l || mp.abs() > l {
        return 0.0;
    }

    let cos_half = (beta * 0.5).cos();
    let sin_half = (beta * 0.5).sin();

    let prefactor = (factorial((l + m) as usize)
        * factorial((l - m) as usize)
        * factorial((l + mp) as usize)
        * factorial((l - mp) as usize))
    .sqrt();

    let k_min = (m - mp).max(0);
    let k_max = (l + m).min(l - mp);

    let mut sum = 0.0;
    for k in k_min..=k_max {
        let denom = factorial((l + m - k) as usize)
            * factorial(k as usize)
            * factorial((mp - m + k) as usize)
            * factorial((l - mp - k) as usize);

        let sign = if (k - m + mp).rem_euclid(2) == 0 {
            1.0
        } else {
            -1.0
        };

        let cos_power = 2 * l + m - mp - 2 * k;
        let sin_power = mp - m + 2 * k;

        sum += sign * prefactor / denom * cos_half.powi(cos_power) * sin_half.powi(sin_power);
    }

    sum
}

fn factorial(n: usize) -> f64 {
    (1..=n).fold(1.0, |acc, value| acc * value as f64)
}

#[cfg(test)]
mod tests {
    use super::rot3i;

    #[test]
    fn rot3i_returns_identity_at_zero_beta() {
        let output = rot3i(2, 2, 0.0).expect("valid dimensions should produce matrices");

        assert!((output.get(1, -1, -1).expect("entry should exist") - 1.0).abs() < 1.0e-12);
        assert!((output.get(1, 0, 0).expect("entry should exist") - 1.0).abs() < 1.0e-12);
        assert!((output.get(1, 1, 1).expect("entry should exist") - 1.0).abs() < 1.0e-12);
        assert!(output.get(1, 1, 0).expect("entry should exist").abs() < 1.0e-12);
    }

    #[test]
    fn rot3i_rows_are_normalized() {
        let output = rot3i(4, 3, 0.7).expect("valid dimensions should produce matrices");
        let matrix = output.matrix_for_l(2).expect("l=2 matrix should exist");

        for row in matrix {
            let norm: f64 = row.iter().map(|value| value * value).sum();
            assert!((norm - 1.0).abs() < 1.0e-9);
        }
    }

    #[test]
    fn rot3i_applies_m_limit_from_mxp1() {
        let output = rot3i(5, 2, 0.3).expect("valid dimensions should produce matrices");

        assert!(output.get(4, 1, -1).is_some());
        assert!(output.get(4, 2, 0).is_none());
    }
}
