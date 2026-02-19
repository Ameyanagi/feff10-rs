use crate::support::math::cwig3j::{Cwig3jError, cwig3j};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreitCoefficients {
    pub cmag: [f64; 3],
    pub cret: [f64; 3],
}

impl Default for BreitCoefficients {
    fn default() -> Self {
        Self {
            cmag: [0.0; 3],
            cret: [0.0; 3],
        }
    }
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum BkmrdfError {
    #[error("orbital index out of range: index={index}, available={available}")]
    OrbitalOutOfRange { index: usize, available: usize },
    #[error("k must be >= 0, got {0}")]
    InvalidK(i32),
    #[error("invalid Wigner 3j input: {0}")]
    Cwig3j(#[from] Cwig3jError),
}

pub fn bkmrdf(i: usize, j: usize, k: i32, kap: &[i32]) -> Result<BreitCoefficients, BkmrdfError> {
    validate_index(i, kap.len())?;
    validate_index(j, kap.len())?;
    if k < 0 {
        return Err(BkmrdfError::InvalidK(k));
    }

    let kap_i = kap[i - 1];
    let kap_j = kap[j - 1];
    let ji = 2 * kap_i.abs() - 1;
    let jj = 2 * kap_j.abs() - 1;
    let kam = kap_j - kap_i;

    let mut output = BreitCoefficients::default();
    let mut l = k - 1;

    for m in 0..3 {
        if l < 0 {
            l += 1;
            continue;
        }

        let mut a = cwig3j(ji, jj, l + l, -1, 1, 2)?;
        a *= a;
        if a == 0.0 {
            l += 1;
            continue;
        }

        let mut c = (l + l + 1) as f64;
        let cm;
        let cz;
        let cp;
        let d;
        let mut cret_terms = None;

        if m == 0 {
            cm = square((kam + k) as f64);
            cz = (kam * kam - k * k) as f64;
            cp = square((k - kam) as f64);
            let n = k as f64;
            let l1 = l + 1;
            let am = ((kam - l) * (kam + l1)) as f64 / c;
            let az = (kam * kam + l * l1) as f64 / c;
            let ap = ((l + kam) * (kam - l1)) as f64 / c;
            d = (k * (k + k + 1)) as f64;
            cret_terms = Some((n, am, az, ap));
        } else if m == 1 {
            cm = square((kap_i + kap_j) as f64);
            cz = cm;
            cp = cm;
            d = (k * (k + 1)) as f64;
        } else {
            cm = square((kam - l) as f64);
            cz = (kam * kam - l * l) as f64;
            cp = square((kam + l) as f64);
            let n = l as f64;
            c = -c;
            let l1 = l + 1;
            let am = ((kam - l) * (kam + l1)) as f64 / c;
            let az = (kam * kam + l * l1) as f64 / c;
            let ap = ((l + kam) * (kam - l1)) as f64 / c;
            d = (l * (k + k + 1)) as f64;
            cret_terms = Some((n, am, az, ap));
        }

        if let Some((n, am, az, ap)) = cret_terms {
            let denom = c.abs() * d;
            let c_term = if denom != 0.0 { n / denom } else { 0.0 };
            output.cret[0] += a * (am - c_term * cm);
            output.cret[1] += (a + a) * (az - c_term * cz);
            output.cret[2] += a * (ap - c_term * cp);
        }

        if d != 0.0 {
            let scaled = a / d;
            output.cmag[0] += cm * scaled;
            output.cmag[1] += cz * (scaled + scaled);
            output.cmag[2] += cp * scaled;
        }

        l += 1;
    }

    Ok(output)
}

fn validate_index(index: usize, available: usize) -> Result<(), BkmrdfError> {
    if index == 0 || index > available {
        return Err(BkmrdfError::OrbitalOutOfRange { index, available });
    }
    Ok(())
}

fn square(value: f64) -> f64 {
    value * value
}

#[cfg(test)]
mod tests {
    use super::{BkmrdfError, bkmrdf};

    #[test]
    fn bkmrdf_rejects_invalid_orbital_index() {
        let error = bkmrdf(2, 1, 1, &[1]).expect_err("index above kap table must fail");
        assert_eq!(
            error,
            BkmrdfError::OrbitalOutOfRange {
                index: 2,
                available: 1
            }
        );
    }

    #[test]
    fn bkmrdf_returns_zero_when_selection_rules_null_all_terms() {
        let value = bkmrdf(1, 2, 5, &[1, 1]).expect("valid inputs should compute");
        assert_eq!(value.cmag, [0.0, 0.0, 0.0]);
        assert_eq!(value.cret, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn bkmrdf_produces_finite_nonzero_coefficients_for_active_channel() {
        let value = bkmrdf(1, 2, 1, &[1, -1]).expect("valid channel should compute");
        assert!(value.cmag.iter().all(|entry| entry.is_finite()));
        assert!(value.cret.iter().all(|entry| entry.is_finite()));
        let magnitude = value
            .cmag
            .iter()
            .chain(value.cret.iter())
            .map(|entry| entry.abs())
            .sum::<f64>();
        assert!(magnitude > 0.0);
    }
}
