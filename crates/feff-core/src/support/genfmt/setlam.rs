use super::m_genfmt::LambdaIndex;

const ONE_DEGREE_RAD: f64 = 0.017_453_292_52;

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum SetlamError {
    #[error("undefined icalc={0}")]
    UndefinedIcalc(i32),
    #[error(
        "computed lambda basis exceeds dimensional limits: mmaxp1={mmaxp1}, nmax={nmax}, mtot={mtot}, ntot={ntot}"
    )]
    DimensionLimitExceeded {
        mmaxp1: usize,
        nmax: usize,
        mtot: usize,
        ntot: usize,
    },
}

#[derive(Debug, Clone)]
pub struct SetlamInput<'a> {
    pub icalc: i32,
    pub ie: i32,
    pub nsc: usize,
    pub nleg: usize,
    pub ilinit: usize,
    pub betas: &'a [f64],
    pub lamtot: usize,
    pub mtot: usize,
    pub ntot: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetlamOutput {
    pub iord: i32,
    pub lambda: Vec<LambdaIndex>,
    pub laml0x: usize,
    pub mmaxp1: usize,
    pub nmax: usize,
    pub truncated: bool,
}

pub fn setlam(input: &SetlamInput<'_>) -> Result<SetlamOutput, SetlamError> {
    let (iord, mmax, nmax_seed) = decode_order_parameters(input)?;

    let mut scratch = Vec::with_capacity(input.lamtot);
    let mut truncated = false;
    let iord_limit = iord.max(0) as usize;

    'outer: for n in 0..=nmax_seed {
        for m in 0..=mmax {
            let jord = 2 * n + m;
            if jord > iord_limit {
                continue;
            }

            if scratch.len() >= input.lamtot {
                truncated = true;
                break 'outer;
            }
            scratch.push(LambdaIndex { m: -(m as i32), n });

            if m == 0 {
                continue;
            }

            if scratch.len() >= input.lamtot {
                truncated = true;
                break 'outer;
            }
            scratch.push(LambdaIndex { m: m as i32, n });
        }
    }

    let mut prioritized = Vec::with_capacity(scratch.len());
    let mut remainder = Vec::with_capacity(scratch.len());
    for lambda in scratch {
        let in_l0_block =
            lambda.n <= input.ilinit && lambda.m.unsigned_abs() as usize <= input.ilinit;
        if in_l0_block {
            prioritized.push(lambda);
        } else {
            remainder.push(lambda);
        }
    }
    let laml0x = prioritized.len();
    prioritized.extend(remainder);

    let mut mmaxp1 = 0_usize;
    let mut nmax = 0_usize;
    for lambda in &prioritized {
        if lambda.m >= 0 {
            mmaxp1 = mmaxp1.max(lambda.m as usize + 1);
        }
        nmax = nmax.max(lambda.n);
    }

    if nmax > input.ntot || mmaxp1 > input.mtot + 1 {
        return Err(SetlamError::DimensionLimitExceeded {
            mmaxp1,
            nmax,
            mtot: input.mtot,
            ntot: input.ntot,
        });
    }

    Ok(SetlamOutput {
        iord,
        lambda: prioritized,
        laml0x,
        mmaxp1,
        nmax,
        truncated,
    })
}

fn decode_order_parameters(input: &SetlamInput<'_>) -> Result<(i32, usize, usize), SetlamError> {
    if input.icalc < 0 {
        let icode = -input.icalc;
        let nmax = (icode % 100) as usize;
        let mmax = ((icode % 10_000) / 100) as usize;
        let iord = icode / 10_000 - 1;
        return Ok((iord, mmax, nmax));
    }

    if input.nsc == 1 {
        let mmax = input.ilinit;
        let nmax = input.ilinit;
        let iord = (2 * nmax + mmax) as i32;
        return Ok((iord, mmax, nmax));
    }

    if input.icalc < 10 {
        let iord = input.icalc;
        let mmax = iord.max(0) as usize;
        let nmax = (iord.max(0) as usize) / 2;
        return Ok((iord, mmax, nmax));
    }

    if input.icalc == 10 {
        let mut mmax = input.ilinit;
        for beta in input.betas.iter().take(input.nleg) {
            let mag1 = beta.abs();
            let mag2 = (mag1 - std::f64::consts::PI).abs();
            if mag1 > ONE_DEGREE_RAD && mag2 > ONE_DEGREE_RAD {
                mmax = 3;
                break;
            }
        }

        let mut nmax = input.ilinit;
        if input.ie >= 42 {
            nmax = 9;
        }
        let iord = (2 * nmax + mmax) as i32;
        return Ok((iord, mmax, nmax));
    }

    Err(SetlamError::UndefinedIcalc(input.icalc))
}

#[cfg(test)]
mod tests {
    use super::{SetlamInput, setlam};

    #[test]
    fn setlam_builds_expected_low_order_basis() {
        let output = setlam(&SetlamInput {
            icalc: 2,
            ie: 1,
            nsc: 2,
            nleg: 2,
            ilinit: 0,
            betas: &[],
            lamtot: 64,
            mtot: 16,
            ntot: 16,
        })
        .expect("valid input should produce lambda basis");

        assert_eq!(output.iord, 2);
        assert_eq!(output.lambda[0].m, 0);
        assert_eq!(output.lambda[0].n, 0);
        assert_eq!(output.laml0x, 1);
        assert!(
            output
                .lambda
                .iter()
                .any(|value| value.m == 2 && value.n == 0)
        );
    }

    #[test]
    fn setlam_decodes_negative_icalc() {
        let output = setlam(&SetlamInput {
            icalc: -10_000,
            ie: 1,
            nsc: 2,
            nleg: 2,
            ilinit: 0,
            betas: &[],
            lamtot: 16,
            mtot: 4,
            ntot: 4,
        })
        .expect("negative icalc encoding should decode");

        assert_eq!(output.iord, 0);
        assert_eq!(
            output.lambda,
            vec![crate::support::genfmt::m_genfmt::LambdaIndex { m: 0, n: 0 }]
        );
    }

    #[test]
    fn setlam_cute_algorithm_detects_non_linear_path() {
        let output = setlam(&SetlamInput {
            icalc: 10,
            ie: 45,
            nsc: 3,
            nleg: 3,
            ilinit: 2,
            betas: &[0.0, 1.0, std::f64::consts::PI],
            lamtot: 512,
            mtot: 32,
            ntot: 32,
        })
        .expect("cute algorithm should produce lambda basis");

        assert_eq!(output.iord, 21);
        assert_eq!(output.nmax, 9);
        assert_eq!(output.mmaxp1, 4);
    }
}
