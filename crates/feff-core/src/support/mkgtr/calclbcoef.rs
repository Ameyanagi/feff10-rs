use crate::support::math::cwig3j::{Cwig3jError, cwig3j};

#[derive(Debug, Clone)]
pub struct ClbCoefTable {
    mjlmax: usize,
    jlmax: usize,
    lx: usize,
    values: Vec<f64>,
}

impl ClbCoefTable {
    fn new(lx: usize, jlmax: usize, mjlmax: usize) -> Self {
        let total = mjlmax * jlmax * 2 * (lx + 1);
        Self {
            mjlmax,
            jlmax,
            lx,
            values: vec![0.0; total],
        }
    }

    pub fn dimensions(&self) -> (usize, usize, usize) {
        (self.lx, self.jlmax, self.mjlmax)
    }

    pub fn get(&self, im: usize, ii: usize, is: usize, ll: usize) -> Option<f64> {
        self.flat_index(im, ii, is, ll)
            .map(|index| self.values[index])
    }

    fn set(&mut self, im: usize, ii: usize, is: usize, ll: usize, value: f64) {
        if let Some(index) = self.flat_index(im, ii, is, ll) {
            self.values[index] = value;
        }
    }

    fn flat_index(&self, im: usize, ii: usize, is: usize, ll: usize) -> Option<usize> {
        if im >= self.mjlmax || ii >= self.jlmax || is > 1 || ll > self.lx {
            return None;
        }

        let ll_stride = self.jlmax * self.mjlmax * 2;
        let is_stride = self.jlmax * self.mjlmax;
        let ii_stride = self.mjlmax;

        Some(ll * ll_stride + is * is_stride + ii * ii_stride + im)
    }
}

pub fn calclbcoef(lx: usize, jlmax: usize, mjlmax: usize) -> Result<ClbCoefTable, Cwig3jError> {
    let mut table = ClbCoefTable::new(lx, jlmax, mjlmax);

    for ll in 0..=lx {
        let lnow = (2 * ll) as i32;

        for is in 0..=1 {
            let ms = 2 * is as i32 - 1;

            for ii in 0..jlmax {
                let jnow = (2 * (ii + 1) - 1) as i32;
                if jnow > (2 * ll + 1) as i32 {
                    continue;
                }

                let im_max = (2 * (ii + 1)).min(mjlmax);
                for im in 0..im_max {
                    let mj = -jnow + 2 * im as i32;
                    let mut coeff = cwig3j(1, jnow, lnow, ms, -mj, 2)?;

                    if ((lnow + mj - 1) / 2).rem_euclid(2) != 0 {
                        coeff = -coeff;
                    }

                    table.set(im, ii, is, ll, coeff);
                }
            }
        }
    }

    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::calclbcoef;
    use crate::support::math::cwig3j::cwig3j;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn dimensions_follow_constructor_arguments() {
        let table = calclbcoef(3, 5, 8).expect("table should build");
        assert_eq!(table.dimensions(), (3, 5, 8));
    }

    #[test]
    fn coefficients_match_cwig3j_reference_formula() {
        let table = calclbcoef(2, 4, 6).expect("table should build");

        let ll = 1_usize;
        let ii = 0_usize;
        let is = 0_usize;
        let im = 0_usize;

        let lnow = (2 * ll) as i32;
        let jnow = (2 * (ii + 1) - 1) as i32;
        let mj = -jnow + 2 * im as i32;
        let ms = 2 * is as i32 - 1;

        let mut expected = cwig3j(1, jnow, lnow, ms, -mj, 2).expect("valid cwig3j input");
        if ((lnow + mj - 1) / 2).rem_euclid(2) != 0 {
            expected = -expected;
        }

        let actual = table
            .get(im, ii, is, ll)
            .expect("coefficient should be in-bounds");
        assert_close(actual, expected, 1.0e-12);
    }

    #[test]
    fn entries_outside_fortran_domain_stay_zero() {
        let table = calclbcoef(2, 4, 6).expect("table should build");

        // ll=0 only supports jnow=1 (ii=0). ii=2 maps to jnow=5 and should remain zero.
        for im in 0..6 {
            let value = table.get(im, 2, 1, 0).expect("in-bounds lookup");
            assert_eq!(value, 0.0);
        }
    }
}
