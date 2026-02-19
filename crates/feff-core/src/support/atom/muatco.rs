use crate::support::math::cwig3j::{Cwig3jError, cwig3j};

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum MuatcoError {
    #[error("norb={norb} exceeds available orbital arrays (xnel={xnel}, xnval={xnval}, kap={kap})")]
    InvalidOrbitalCount {
        norb: usize,
        xnel: usize,
        xnval: usize,
        kap: usize,
    },
    #[error("orbital {index} has invalid kappa=0")]
    InvalidKappa { index: usize },
    #[error("invalid Wigner 3j input: {0}")]
    Cwig3j(#[from] Cwig3jError),
}

pub fn muatco(
    xnel: &[f64],
    xnval: &[f64],
    kap: &[i32],
    norb: usize,
) -> Result<Vec<Vec<Vec<f64>>>, MuatcoError> {
    if norb > xnel.len() || norb > xnval.len() || norb > kap.len() {
        return Err(MuatcoError::InvalidOrbitalCount {
            norb,
            xnel: xnel.len(),
            xnval: xnval.len(),
            kap: kap.len(),
        });
    }

    let mut afgk = vec![vec![vec![0.0_f64; 5]; norb]; norb];

    let mut i = 0usize;
    while i < norb {
        if kap[i] == 0 {
            return Err(MuatcoError::InvalidKappa { index: i + 1 });
        }
        let li = kap[i].abs() * 2 - 1;

        let mut j = 0usize;
        while j <= i {
            if kap[j] == 0 {
                return Err(MuatcoError::InvalidKappa { index: j + 1 });
            }

            let lj = kap[j].abs() * 2 - 1;
            let kmax = (li + lj) / 2;
            let mut kmin = (li - lj).abs() / 2;
            if kap[i] * kap[j] < 0 {
                kmin += 1;
            }

            let mut m = 0.0;
            if j == i && xnval[i] <= 0.0 {
                m = 1.0;
            }
            afgk[j][i][0] += xnel[i] * (xnel[j] - m);

            if !(xnval[i] > 0.0 && xnval[j] > 0.0) {
                let mut b = afgk[j][i][0];
                if j == i && xnval[i] <= 0.0 {
                    let a = li as f64;
                    b = -b * (a + 1.0) / a;
                    kmin += 2;
                }

                let mut k = kmin;
                while k <= kmax {
                    let k_half = (k / 2) as usize;
                    if k_half < 5 {
                        let coeff = cwig3j(li, k + k, lj, 1, 0, 2)?;
                        afgk[i][j][k_half] = b * coeff * coeff;
                    }
                    k += 2;
                }
            }

            j += 1;
        }

        i += 1;
    }

    Ok(afgk)
}

#[cfg(test)]
mod tests {
    use super::muatco;
    use crate::support::math::cwig3j::cwig3j;

    #[test]
    fn muatco_builds_a_and_b_tables_with_fortran_indexing() {
        let xnel = [2.0, 1.0];
        let xnval = [0.0, 0.0];
        let kap = [1, -1];

        let table = muatco(&xnel, &xnval, &kap, 2).expect("muatco should succeed");

        assert!((table[0][0][0] - 2.0).abs() <= 1.0e-12);
        assert!((table[0][1][0] - 2.0).abs() <= 1.0e-12);

        let w = cwig3j(1, 2, 1, 1, 0, 2).expect("wigner value should be valid");
        let expected = 2.0 * w * w;
        assert!((table[1][0][0] - expected).abs() <= 1.0e-12);
    }

    #[test]
    fn muatco_skips_exchange_coefficients_for_valence_valence_pairs() {
        let xnel = [1.0, 1.0];
        let xnval = [1.0, 1.0];
        let kap = [1, 1];

        let table = muatco(&xnel, &xnval, &kap, 2).expect("muatco should succeed");
        assert_eq!(table[1][0][0], 0.0);
        assert_eq!(table[1][0][1], 0.0);
        assert_eq!(table[1][0][2], 0.0);
    }
}
