pub type AngularCoeffFn = dyn Fn(usize, usize, i32) -> f64;
pub type RadialIntegralFn = dyn Fn(usize, usize, usize, usize, i32) -> f64;

#[derive(Clone)]
pub struct LagdatInput<'a> {
    pub ia: i32,
    pub include_exchange: bool,
    pub norbsc: usize,
    pub xnel: &'a [f64],
    pub kap: &'a [i32],
    pub nre: &'a [i32],
    pub akeato: &'a AngularCoeffFn,
    pub bkeato: &'a AngularCoeffFn,
    pub fdrirk: &'a RadialIntegralFn,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum LagdatError {
    #[error("norbsc must be in 1..={available}, got {norbsc}")]
    InvalidOrbitalCount { norbsc: usize, available: usize },
    #[error("eps length must be at least {required}, got {got}")]
    EpsLengthMismatch { required: usize, got: usize },
    #[error("orbital occupation xnel({index}) is zero; cannot divide")]
    ZeroOccupation { index: usize },
    #[error("ia must be in [-norbsc, norbsc], got {ia}")]
    InvalidIa { ia: i32 },
}

pub fn lagdat(input: &LagdatInput<'_>, eps: &mut [f64]) -> Result<(), LagdatError> {
    let available = input.xnel.len().min(input.kap.len()).min(input.nre.len());
    if input.norbsc == 0 || input.norbsc > available {
        return Err(LagdatError::InvalidOrbitalCount {
            norbsc: input.norbsc,
            available,
        });
    }

    if input.ia.unsigned_abs() as usize > input.norbsc {
        return Err(LagdatError::InvalidIa { ia: input.ia });
    }

    let required_eps = input.norbsc.saturating_mul(input.norbsc.saturating_sub(1)) / 2;
    if eps.len() < required_eps {
        return Err(LagdatError::EpsLengthMismatch {
            required: required_eps,
            got: eps.len(),
        });
    }

    let mut i1 = input.ia.max(1) as usize;

    loop {
        let idep = if input.ia > 0 { 1 } else { i1 + 1 };
        if i1 > input.norbsc {
            break;
        }

        let ji1 = 2 * input.kap[i1 - 1].abs() - 1;

        let mut i2 = idep;
        while i2 <= input.norbsc {
            if i2 != i1
                && input.kap[i2 - 1] == input.kap[i1 - 1]
                && !(input.nre[i1 - 1] < 0 && input.nre[i2 - 1] < 0)
                && input.xnel[i1 - 1] != input.xnel[i2 - 1]
            {
                if input.xnel[i1 - 1] == 0.0 {
                    return Err(LagdatError::ZeroOccupation { index: i1 });
                }
                if input.xnel[i2 - 1] == 0.0 {
                    return Err(LagdatError::ZeroOccupation { index: i2 });
                }

                let mut d = 0.0_f64;
                let mut l = 1usize;
                while l <= input.norbsc {
                    let jjl = 2 * input.kap[l - 1].abs() - 1;
                    let mut k = 0_i32;
                    let kma = ji1.min(jjl);

                    while k <= kma {
                        let a = (input.akeato)(l, i1, k) / input.xnel[i1 - 1];
                        let b = a - (input.akeato)(l, i2, k) / input.xnel[i2 - 1];
                        let c = if a != 0.0 { b / a } else { b };
                        if c.abs() >= 1.0e-7 {
                            d += b * (input.fdrirk)(l, l, i1, i2, k);
                        }
                        k += 2;
                    }

                    if input.include_exchange {
                        let kma_ex = (ji1 + jjl) / 2;
                        let mut k_ex = (jjl - kma_ex).abs();
                        if input.kap[i1 - 1] * input.kap[l - 1] < 0 {
                            k_ex += 1;
                        }

                        while k_ex <= kma_ex {
                            let a = (input.bkeato)(l, i2, k_ex) / input.xnel[i2 - 1];
                            let b = a - (input.bkeato)(l, i1, k_ex) / input.xnel[i1 - 1];
                            let c = if a != 0.0 { b / a } else { b };
                            if c.abs() >= 1.0e-7 {
                                d += b * (input.fdrirk)(i1, l, i2, l, k_ex);
                            }
                            k_ex += 2;
                        }
                    }

                    l += 1;
                }

                let i = i1.min(i2);
                let j = i1.max(i2);
                let idx = packed_lower_index(i, j);
                eps[idx] = d / (input.xnel[i2 - 1] - input.xnel[i1 - 1]);
            }

            i2 += 1;
        }

        if input.ia > 0 {
            break;
        }

        i1 += 1;
        if i1 >= input.norbsc {
            break;
        }
    }

    Ok(())
}

fn packed_lower_index(i: usize, j: usize) -> usize {
    (i - 1) + ((j - 1) * (j - 2)) / 2
}

#[cfg(test)]
mod tests {
    use super::{LagdatInput, lagdat};

    #[test]
    fn lagdat_updates_requested_pair_for_single_orbital_target() {
        let mut eps = vec![0.0; 3];
        let xnel = [1.0, 3.0, 5.0];
        let kap = [1, 1, 1];
        let nre = [1, 1, 1];

        let input = LagdatInput {
            ia: 1,
            include_exchange: false,
            norbsc: 3,
            xnel: &xnel,
            kap: &kap,
            nre: &nre,
            akeato: &|_, i, _| if i == 1 { 1.0 } else { 2.0 },
            bkeato: &|_, _, _| 0.0,
            fdrirk: &|_, _, _, _, _| 1.0,
        };

        lagdat(&input, &mut eps).expect("lagdat should succeed");

        assert!((eps[0] - 0.5).abs() <= 1.0e-12);
        assert!(eps[1].abs() > 0.0);
        assert_eq!(eps[2], 0.0);
    }

    #[test]
    fn lagdat_can_compute_all_pairs_when_ia_is_non_positive() {
        let mut eps = vec![0.0; 3];
        let xnel = [1.0, 2.0, 4.0];
        let kap = [1, 1, 1];
        let nre = [1, 1, 1];

        let input = LagdatInput {
            ia: 0,
            include_exchange: true,
            norbsc: 3,
            xnel: &xnel,
            kap: &kap,
            nre: &nre,
            akeato: &|_, _, _| 1.0,
            bkeato: &|_, _, _| 1.0,
            fdrirk: &|_, _, _, _, _| 0.5,
        };

        lagdat(&input, &mut eps).expect("lagdat should succeed");

        assert!(eps.iter().all(|value| value.is_finite()));
    }
}
