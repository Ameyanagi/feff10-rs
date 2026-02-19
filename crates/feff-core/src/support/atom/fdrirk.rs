pub type YzkrdfFn = dyn FnMut(i32, i32, i32, &mut [f64], &mut [f64]);
pub type DsordfFn = dyn Fn(i32, i32, i32, i32, f64, &[f64], &[f64]) -> f64;

#[derive(Debug, Clone)]
pub struct FdrirkContext {
    pub kap: Vec<i32>,
    pub ag: Vec<f64>,
    pub ap: Vec<f64>,
    pub ndor: usize,
    pub nem: i32,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum FdrirkError {
    #[error("ndor must be >= 1")]
    InvalidNdor,
    #[error("ag/ap lengths must be >= ndor (ag={ag}, ap={ap}, ndor={ndor})")]
    CoefficientLengthMismatch { ag: usize, ap: usize, ndor: usize },
    #[error("orbital index {index} is outside 1..={available}")]
    OrbitalOutOfRange { index: i32, available: usize },
}

#[allow(clippy::too_many_arguments)]
pub fn fdrirk(
    i: i32,
    j: i32,
    l: i32,
    m: i32,
    k: i32,
    context: &mut FdrirkContext,
    yzkrdf: &mut YzkrdfFn,
    dsordf: &DsordfFn,
) -> Result<f64, FdrirkError> {
    if context.ndor == 0 {
        return Err(FdrirkError::InvalidNdor);
    }
    if context.ag.len() < context.ndor || context.ap.len() < context.ndor {
        return Err(FdrirkError::CoefficientLengthMismatch {
            ag: context.ag.len(),
            ap: context.ap.len(),
            ndor: context.ndor,
        });
    }

    let mut a = (k + 1) as f64;

    if i > 0 && j > 0 {
        validate_orbital(i, context.kap.len())?;
        validate_orbital(j, context.kap.len())?;

        yzkrdf(
            i,
            j,
            k,
            &mut context.ag[..context.ndor],
            &mut context.ap[..context.ndor],
        );

        let mut nn = context.kap[(i - 1) as usize].abs() + context.kap[(j - 1) as usize].abs();
        nn = (nn - k).max(1);
        a = (k + 1) as f64;

        let mut hg = vec![0.0_f64; context.ndor];
        let mut n = 0usize;
        while n < context.ndor {
            if nn <= context.ndor as i32 {
                hg[(nn - 1) as usize] = -context.ag[n];
            }
            nn += 1;
            n += 1;
        }

        context.ag[..context.ndor].copy_from_slice(&hg);
        context.ag[0] += context.ap[0];
    }

    if l <= 0 || m <= 0 {
        return Ok(0.0);
    }

    let n = if context.nem != 0 { -2 } else { -1 };
    Ok(dsordf(
        l,
        m,
        -1,
        n,
        a,
        &context.ag[..context.ndor],
        &context.ap[..context.ndor],
    ))
}

fn validate_orbital(index: i32, available: usize) -> Result<(), FdrirkError> {
    if index <= 0 || index as usize > available {
        return Err(FdrirkError::OrbitalOutOfRange { index, available });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{FdrirkContext, FdrirkError, fdrirk};

    #[test]
    fn fdrirk_reorders_origin_coefficients_before_dsordf_call() {
        let mut context = FdrirkContext {
            kap: vec![1, 2],
            ag: vec![0.0, 0.0, 0.0],
            ap: vec![0.0, 0.0, 0.0],
            ndor: 3,
            nem: 0,
        };

        let mut yzkrdf = |_: i32, _: i32, _: i32, ag: &mut [f64], ap: &mut [f64]| {
            ag.copy_from_slice(&[1.0, 2.0, 3.0]);
            ap.copy_from_slice(&[0.5, 0.0, 0.0]);
        };

        let value = fdrirk(
            1,
            2,
            1,
            1,
            1,
            &mut context,
            &mut yzkrdf,
            &|_, _, _, n, _, _, _| n as f64,
        )
        .expect("fdrirk should succeed");

        assert!((context.ag[0] - 0.5).abs() <= 1.0e-12);
        assert!((context.ag[1] + 1.0).abs() <= 1.0e-12);
        assert!((context.ag[2] + 2.0).abs() <= 1.0e-12);
        assert_eq!(value, -1.0);
    }

    #[test]
    fn fdrirk_switches_dsordf_mode_for_exchange_case() {
        let mut context = FdrirkContext {
            kap: vec![1, 1],
            ag: vec![0.0, 0.0],
            ap: vec![0.0, 0.0],
            ndor: 2,
            nem: 3,
        };

        let mut yzkrdf = |_: i32, _: i32, _: i32, ag: &mut [f64], ap: &mut [f64]| {
            ag.copy_from_slice(&[0.0, 0.0]);
            ap.copy_from_slice(&[0.0, 0.0]);
        };

        let value = fdrirk(
            1,
            1,
            1,
            1,
            0,
            &mut context,
            &mut yzkrdf,
            &|_, _, _, n, _, _, _| n as f64,
        )
        .expect("fdrirk should succeed");

        assert_eq!(value, -2.0);
    }

    #[test]
    fn fdrirk_rejects_out_of_range_orbitals() {
        let mut context = FdrirkContext {
            kap: vec![1],
            ag: vec![0.0],
            ap: vec![0.0],
            ndor: 1,
            nem: 0,
        };

        let mut yzkrdf = |_: i32, _: i32, _: i32, _: &mut [f64], _: &mut [f64]| {};
        let error = fdrirk(
            2,
            1,
            1,
            1,
            0,
            &mut context,
            &mut yzkrdf,
            &|_, _, _, _, _, _, _| 0.0,
        )
        .expect_err("invalid orbital index should fail");

        assert_eq!(
            error,
            FdrirkError::OrbitalOutOfRange {
                index: 2,
                available: 1
            }
        );
    }
}
