//! FEFF COMMON physical constants ported from `m_constants.f90`.
//!
//! These values are shared across migrated physics kernels to avoid ad hoc
//! per-module literal constants.

pub const PI2: f64 = 6.283_185_307_179_586_476_925_286_766_559_f64;
pub const PI: f64 = 3.141_592_653_589_793_238_462_643_383_279_5_f64;
pub const ONE: f64 = 1.0;
pub const ZERO: f64 = 0.0;
pub const THIRD: f64 = 1.0 / 3.0;
pub const TWO_THIRDS: f64 = 2.0 / 3.0;
pub const RADDEG: f64 = 180.0 / PI;
pub const FA: f64 = 1.919_158_292_677_512_811_f64;
pub const BOHR: f64 = 0.529_177_249_f64;
pub const RYD: f64 = 13.605_698_f64;
pub const HART: f64 = 2.0 * RYD;
pub const ALPINV: f64 = 137.035_989_56_f64;
pub const ALPHFS: f64 = 1.0 / ALPINV;
pub const EV2RY: f64 = 1.0 / 13.6058_f64;
pub const HBARC_EV: f64 = 1_973.2708_f64 / 0.529_177_f64;
pub const HBARC_ATOMIC: f64 = 137.041_88_f64;
pub const MEC2: f64 = 511_004.0_f64;
pub const H_ON_SQRT_TWO_ME: f64 = 23.1761_f64;
pub const MEC_ON_HBAR: f64 = 137.041_88_f64;

#[cfg(test)]
mod tests {
    use super::{
        ALPHFS, ALPINV, BOHR, EV2RY, FA, H_ON_SQRT_TWO_ME, HART, HBARC_ATOMIC, HBARC_EV,
        MEC_ON_HBAR, MEC2, ONE, PI, PI2, RADDEG, RYD, THIRD, TWO_THIRDS, ZERO,
    };

    #[test]
    fn constants_match_expected_relationships() {
        assert_eq!(ONE, 1.0);
        assert_eq!(ZERO, 0.0);
        assert_eq!(THIRD, 1.0 / 3.0);
        assert_eq!(TWO_THIRDS, 2.0 / 3.0);

        assert!((PI2 - 2.0 * PI).abs() <= 1.0e-15);
        assert!((RADDEG * PI - 180.0).abs() <= 1.0e-12);
        assert!((HART - 2.0 * RYD).abs() <= f64::EPSILON);
        assert!((ALPHFS - 1.0 / ALPINV).abs() <= f64::EPSILON);
    }

    #[test]
    fn physics_constants_remain_finite_and_positive() {
        for value in [
            BOHR,
            FA,
            RYD,
            HBARC_EV,
            HBARC_ATOMIC,
            MEC2,
            H_ON_SQRT_TWO_ME,
            MEC_ON_HBAR,
            EV2RY,
        ] {
            assert!(value.is_finite());
            assert!(value > 0.0);
        }
    }
}
