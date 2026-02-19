pub const DEFAULT_H_ON_SQRT_TWO_ME_AU: f64 = 23.176091881684552;
pub const DEFAULT_ME_C2_EV: f64 = 511_004.0;

#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq)]
pub enum MdffWavelengthError {
    #[error("energy must be finite and positive, got {value}")]
    InvalidEnergy { value: f64 },
    #[error("h_on_sqrt_two_me must be finite and positive, got {value}")]
    InvalidPrefactor { value: f64 },
    #[error("me_c2_ev must be finite and positive, got {value}")]
    InvalidMeC2 { value: f64 },
}

pub fn mdff_wavelength(energy_ev: f64) -> Result<f64, MdffWavelengthError> {
    mdff_wavelength_with_constants(energy_ev, DEFAULT_H_ON_SQRT_TWO_ME_AU, DEFAULT_ME_C2_EV)
}

pub fn mdff_wavelength_with_constants(
    energy_ev: f64,
    h_on_sqrt_two_me: f64,
    me_c2_ev: f64,
) -> Result<f64, MdffWavelengthError> {
    if !energy_ev.is_finite() || energy_ev <= 0.0 {
        return Err(MdffWavelengthError::InvalidEnergy { value: energy_ev });
    }
    if !h_on_sqrt_two_me.is_finite() || h_on_sqrt_two_me <= 0.0 {
        return Err(MdffWavelengthError::InvalidPrefactor {
            value: h_on_sqrt_two_me,
        });
    }
    if !me_c2_ev.is_finite() || me_c2_ev <= 0.0 {
        return Err(MdffWavelengthError::InvalidMeC2 { value: me_c2_ev });
    }

    let denominator = (energy_ev + energy_ev * energy_ev / (2.0 * me_c2_ev)).sqrt();
    Ok(h_on_sqrt_two_me / denominator)
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_H_ON_SQRT_TWO_ME_AU, DEFAULT_ME_C2_EV, MdffWavelengthError, mdff_wavelength,
        mdff_wavelength_with_constants,
    };

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn wavelength_matches_reference_formula() {
        let value = mdff_wavelength(200_000.0).expect("wavelength should be finite");
        let expected = DEFAULT_H_ON_SQRT_TWO_ME_AU
            / (200_000.0_f64 + 200_000.0_f64 * 200_000.0_f64 / (2.0 * DEFAULT_ME_C2_EV)).sqrt();

        assert_close(value, expected, 1.0e-12);
    }

    #[test]
    fn wavelength_decreases_with_beam_energy() {
        let low = mdff_wavelength(80_000.0).expect("low-energy wavelength");
        let high = mdff_wavelength(300_000.0).expect("high-energy wavelength");

        assert!(high < low);
    }

    #[test]
    fn rejects_invalid_inputs() {
        let error = mdff_wavelength(0.0).expect_err("non-positive energy should fail");
        assert_eq!(error, MdffWavelengthError::InvalidEnergy { value: 0.0 });

        let error = mdff_wavelength_with_constants(100.0, -1.0, DEFAULT_ME_C2_EV)
            .expect_err("negative prefactor should fail");
        assert_eq!(error, MdffWavelengthError::InvalidPrefactor { value: -1.0 });
    }
}
