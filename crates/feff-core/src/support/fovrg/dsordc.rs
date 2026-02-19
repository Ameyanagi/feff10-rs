use super::aprdec::aprdec;
use num_complex::Complex64;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum DsordcError {
    #[error("dg/dp/cg/cp/dr inputs must share the same non-zero length")]
    LengthMismatch,
    #[error("simpson integration requires odd number of radial points >= 3, got {0}")]
    InvalidRadialPointCount(usize),
    #[error("ndor must be at least 1")]
    InvalidNdor,
}

#[derive(Debug, Clone)]
pub struct DsordcInput<'a> {
    pub a: f64,
    pub fl_j: f64,
    pub dg: &'a [Complex64],
    pub dp: &'a [Complex64],
    pub cg_j: &'a [Complex64],
    pub cp_j: &'a [Complex64],
    pub dr: &'a [f64],
    pub hx: f64,
    pub ag: &'a [Complex64],
    pub bg_j: &'a [Complex64],
    pub ap: &'a [Complex64],
    pub bp_j: &'a [Complex64],
    pub ndor: usize,
}

pub fn dsordc(input: &DsordcInput<'_>) -> Result<Complex64, DsordcError> {
    let idim = input.dg.len();
    if idim == 0
        || input.dp.len() != idim
        || input.cg_j.len() != idim
        || input.cp_j.len() != idim
        || input.dr.len() != idim
    {
        return Err(DsordcError::LengthMismatch);
    }
    if idim < 3 || idim.is_multiple_of(2) {
        return Err(DsordcError::InvalidRadialPointCount(idim));
    }

    let ndor = input
        .ndor
        .min(input.ag.len())
        .min(input.bg_j.len())
        .min(input.ap.len())
        .min(input.bp_j.len());
    if ndor == 0 {
        return Err(DsordcError::InvalidNdor);
    }

    let mut hg = Vec::with_capacity(idim);
    for i in 0..idim {
        hg.push((input.dg[i] * input.cg_j[i] + input.dp[i] * input.cp_j[i]) * input.dr[i]);
    }

    let mut integral = Complex64::new(0.0, 0.0);
    let mut l = 1_usize;
    while l < idim - 1 {
        integral += hg[l] + hg[l] + hg[l + 1];
        l += 2;
    }
    integral = input.hx * (integral + integral + hg[0] - hg[idim - 1]) / 3.0;

    let mut b = input.a + input.fl_j;
    for l in 1..=ndor {
        b += 1.0;
        let chg = aprdec(input.ag, input.bg_j, l) + aprdec(input.ap, input.bp_j, l);
        integral += chg * input.dr[0].powf(b) / b;
    }

    Ok(integral)
}

#[cfg(test)]
mod tests {
    use super::{DsordcError, DsordcInput, dsordc};
    use num_complex::Complex64;

    fn c(value: f64) -> Complex64 {
        Complex64::new(value, 0.0)
    }

    #[test]
    fn dsordc_accumulates_simpson_and_origin_terms() {
        let input = DsordcInput {
            a: 0.0,
            fl_j: 0.0,
            dg: &[c(1.0), c(1.0), c(1.0), c(1.0), c(1.0)],
            dp: &[c(0.0), c(0.0), c(0.0), c(0.0), c(0.0)],
            cg_j: &[c(1.0), c(1.0), c(1.0), c(1.0), c(1.0)],
            cp_j: &[c(0.0), c(0.0), c(0.0), c(0.0), c(0.0)],
            dr: &[1.0, 2.0, 3.0, 4.0, 5.0],
            hx: 1.0,
            ag: &[c(1.0)],
            bg_j: &[c(2.0)],
            ap: &[c(0.0)],
            bp_j: &[c(0.0)],
            ndor: 1,
        };

        let value = dsordc(&input).expect("valid dsordc input should succeed");
        assert!((value.re - 14.0).abs() <= 1.0e-12);
        assert!(value.im.abs() <= 1.0e-12);
    }

    #[test]
    fn dsordc_rejects_even_grid() {
        let input = DsordcInput {
            a: 0.0,
            fl_j: 0.0,
            dg: &[c(1.0), c(1.0), c(1.0), c(1.0)],
            dp: &[c(0.0), c(0.0), c(0.0), c(0.0)],
            cg_j: &[c(1.0), c(1.0), c(1.0), c(1.0)],
            cp_j: &[c(0.0), c(0.0), c(0.0), c(0.0)],
            dr: &[1.0, 2.0, 3.0, 4.0],
            hx: 1.0,
            ag: &[c(1.0)],
            bg_j: &[c(1.0)],
            ap: &[c(0.0)],
            bp_j: &[c(0.0)],
            ndor: 1,
        };

        let error = dsordc(&input).expect_err("even point count should fail");
        assert_eq!(error, DsordcError::InvalidRadialPointCount(4));
    }
}
