use super::bkmrdf::BreitCoefficients;

pub type FdrirkFn = dyn Fn(i32, i32, i32, i32, i32) -> f64;
pub type AngularCoeffFn = dyn Fn(usize, usize, i32) -> f64;
pub type OccupancyFn = dyn Fn(usize, usize) -> f64;
pub type BreitCoeffFn = dyn Fn(usize, usize, i32) -> BreitCoefficients;

pub struct EtotalInput<'a> {
    pub kap: &'a [i32],
    pub xnel: &'a [f64],
    pub xnval: &'a [f64],
    pub en: &'a [f64],
    pub norb: usize,
    pub fdrirk: &'a FdrirkFn,
    pub akeato: &'a AngularCoeffFn,
    pub bkeato: &'a AngularCoeffFn,
    pub fdmocc: &'a OccupancyFn,
    pub bkmrdf: &'a BreitCoeffFn,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EtotalBreakdown {
    pub coulomb_direct: f64,
    pub coulomb_exchange: f64,
    pub magnetic: f64,
    pub retardation: f64,
    pub total: f64,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum EtotalError {
    #[error("norb={norb} exceeds available orbital data length={available}")]
    InvalidOrbitalCount { norb: usize, available: usize },
}

pub fn etotal(input: &EtotalInput<'_>) -> Result<EtotalBreakdown, EtotalError> {
    let available = input
        .kap
        .len()
        .min(input.xnel.len())
        .min(input.xnval.len())
        .min(input.en.len());
    if input.norb == 0 || input.norb > available {
        return Err(EtotalError::InvalidOrbitalCount {
            norb: input.norb,
            available,
        });
    }

    let mut ener = [0.0_f64; 4];

    for i in 1..=input.norb {
        let l = input.kap[i - 1].abs() - 1;
        for j in 1..=i {
            let mut a = 1.0;
            if j == i {
                a += a;
            }
            let m = input.kap[j - 1].abs() - 1;
            let kmi = 2 * l.min(m);
            let mut k = 0;
            while k <= kmi {
                let cer = (input.fdrirk)(i as i32, i as i32, j as i32, j as i32, k);
                ener[0] += cer * (input.akeato)(i, j, k) / a;
                k += 2;
            }
        }
    }

    if input.norb > 1 {
        for i in 2..=input.norb {
            let mut a = 1.0;
            if input.xnval[i - 1] > 0.0 {
                a = 0.5;
            }
            for j in 1..=(i - 1) {
                if input.xnval[j - 1] > 0.0 {
                    continue;
                }
                let l = input.kap[i - 1].abs();
                let m = input.kap[j - 1].abs();
                let mut k = (l - m).abs();
                if input.kap[i - 1] * input.kap[j - 1] < 0 {
                    k += 1;
                }
                let kmi = l + m - 1;
                while k <= kmi {
                    let cer = (input.fdrirk)(i as i32, j as i32, i as i32, j as i32, k);
                    ener[1] -= cer * (input.bkeato)(i, j, k) * a;
                    k += 2;
                }
            }
        }
    }

    for j in 1..=input.norb {
        let jj = 2 * input.kap[j - 1].abs() - 1;
        for i in 1..=j {
            let ji = 2 * input.kap[i - 1].abs() - 1;
            let mut k = 1;
            let kma = ji.min(jj);
            while k <= kma {
                let cer = (input.fdrirk)(j as i32, j as i32, i as i32, i as i32, k);
                if i == j {
                    let coeffs = (input.bkmrdf)(j, j, k);
                    let occ = (input.fdmocc)(j, j);
                    ener[2] += (coeffs.cmag[0] + coeffs.cmag[1] + coeffs.cmag[2]) * cer * occ / 2.0;
                }
                k += 2;
            }
        }
    }

    if input.norb > 1 {
        for j in 2..=input.norb {
            let kap_j = input.kap[j - 1];
            let mut lj = kap_j.abs();
            let mut na = -1;
            if kap_j <= 0 {
                na = -na;
                lj -= 1;
            }

            for l_index in 1..=(j - 1) {
                let kap_l = input.kap[l_index - 1];
                let mut ll = kap_l.abs();
                let mut nb = -1;
                if kap_l <= 0 {
                    nb = -nb;
                    ll -= 1;
                }

                let b = (input.fdmocc)(j, l_index);
                let nm1 = (lj + na - ll).abs();
                let nmp1 = ll + lj + nb;
                let nmm1 = ll + lj + na;
                let np1 = (ll + nb - lj).abs();

                let mut k = nm1.min(np1);
                let kma = nmp1.max(nmm1);
                if (k + ll + lj) % 2 == 0 {
                    k += 1;
                }

                let nb_total = kap_j.abs() + kap_l.abs();
                while k <= kma {
                    let coeffs = (input.bkmrdf)(j, l_index, k);
                    let mut cer = [0.0_f64; 3];

                    if !(nb_total <= k && kap_l < 0 && kap_j > 0) {
                        cer[0] =
                            (input.fdrirk)(l_index as i32, j as i32, l_index as i32, j as i32, k);
                        cer[1] = (input.fdrirk)(0, 0, j as i32, l_index as i32, k);
                    }

                    if !(nb_total <= k && kap_l > 0 && kap_j < 0) {
                        cer[2] =
                            (input.fdrirk)(j as i32, l_index as i32, j as i32, l_index as i32, k);
                        if cer[1] == 0.0 {
                            cer[1] = (input.fdrirk)(0, 0, l_index as i32, j as i32, k);
                        }
                    }

                    for (idx, cer_value) in cer.iter().enumerate() {
                        ener[2] += coeffs.cmag[idx] * cer_value * b;
                        ener[3] += coeffs.cret[idx] * cer_value * b;
                    }

                    k += 2;
                }
            }
        }
    }

    let mut total = -(ener[0] + ener[1]) + ener[2] + ener[3];
    for idx in 0..input.norb {
        total += input.en[idx] * input.xnel[idx];
    }

    Ok(EtotalBreakdown {
        coulomb_direct: ener[0],
        coulomb_exchange: ener[1],
        magnetic: ener[2],
        retardation: ener[3],
        total,
    })
}

#[cfg(test)]
mod tests {
    use super::{EtotalBreakdown, EtotalError, EtotalInput, etotal};
    use crate::support::atom::bkmrdf::BreitCoefficients;

    #[test]
    fn etotal_handles_single_orbital_case() {
        let input = EtotalInput {
            kap: &[1],
            xnel: &[2.0],
            xnval: &[0.0],
            en: &[3.0],
            norb: 1,
            fdrirk: &|_, _, _, _, k| {
                if k == 0 { 4.0 } else { 5.0 }
            },
            akeato: &|_, _, _| 2.0,
            bkeato: &|_, _, _| 0.0,
            fdmocc: &|_, _| 2.0,
            bkmrdf: &|_, _, _| BreitCoefficients {
                cmag: [1.0, 2.0, 3.0],
                cret: [0.0, 0.0, 0.0],
            },
        };

        let breakdown = etotal(&input).expect("single orbital etotal should succeed");
        assert_eq!(
            breakdown,
            EtotalBreakdown {
                coulomb_direct: 4.0,
                coulomb_exchange: 0.0,
                magnetic: 30.0,
                retardation: 0.0,
                total: 32.0,
            }
        );
    }

    #[test]
    fn etotal_can_reduce_to_orbital_energy_sum_when_interactions_are_zero() {
        let input = EtotalInput {
            kap: &[1, -1],
            xnel: &[1.0, 1.0],
            xnval: &[0.0, 0.0],
            en: &[2.0, 3.0],
            norb: 2,
            fdrirk: &|_, _, _, _, _| 0.0,
            akeato: &|_, _, _| 0.0,
            bkeato: &|_, _, _| 0.0,
            fdmocc: &|_, _| 0.0,
            bkmrdf: &|_, _, _| BreitCoefficients {
                cmag: [0.0, 0.0, 0.0],
                cret: [0.0, 0.0, 0.0],
            },
        };

        let breakdown = etotal(&input).expect("zero interaction test should succeed");
        assert!((breakdown.total - 5.0).abs() <= 1.0e-12);
    }

    #[test]
    fn etotal_rejects_invalid_orbital_count() {
        let input = EtotalInput {
            kap: &[1],
            xnel: &[2.0],
            xnval: &[0.0],
            en: &[3.0],
            norb: 2,
            fdrirk: &|_, _, _, _, _| 0.0,
            akeato: &|_, _, _| 0.0,
            bkeato: &|_, _, _| 0.0,
            fdmocc: &|_, _| 0.0,
            bkmrdf: &|_, _, _| BreitCoefficients {
                cmag: [0.0, 0.0, 0.0],
                cret: [0.0, 0.0, 0.0],
            },
        };

        let error = etotal(&input).expect_err("invalid norb should fail");
        assert_eq!(
            error,
            EtotalError::InvalidOrbitalCount {
                norb: 2,
                available: 1
            }
        );
    }
}
