pub const DELTA: f64 = 0.050_000_000_000_000;
pub const C88: f64 = 8.8;

pub fn xx(j: i32) -> f64 {
    -C88 + (j as f64 - 1.0) * DELTA
}

pub fn rr(j: i32) -> f64 {
    xx(j).exp()
}

pub fn ii(r: f64) -> i32 {
    ((r.ln() + C88) / DELTA + 1.0).trunc() as i32
}

#[cfg(test)]
mod tests {
    use super::{ii, rr, xx};

    #[test]
    fn xx_matches_fortran_grid_origin() {
        assert!((xx(1) + 8.8).abs() < 1.0e-12);
        assert!((xx(2) + 8.75).abs() < 1.0e-12);
    }

    #[test]
    fn rr_is_exp_of_xx() {
        let j = 77;
        assert!((rr(j) - xx(j).exp()).abs() < 1.0e-12);
    }

    #[test]
    fn ii_inverts_rr_for_positive_grid_points() {
        for j in [1, 2, 50, 120] {
            assert_eq!(ii(rr(j)), j);
        }
    }
}
