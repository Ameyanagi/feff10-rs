use num_complex::Complex64;
use std::f64::consts::PI;

const SMALL_T_CUTOFF: f64 = 0.1;
const HIGH_ENERGY_EXTENSION_FACTOR: f64 = 50.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpectralInterpolationInput<'a> {
    pub energy: f64,
    pub energy_grid: &'a [f64],
    pub spectrum: &'a [Complex64],
}

impl<'a> SpectralInterpolationInput<'a> {
    pub fn new(energy: f64, energy_grid: &'a [f64], spectrum: &'a [Complex64]) -> Self {
        Self {
            energy,
            energy_grid,
            spectrum,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LorentzianConvolutionInput<'a> {
    pub energy_grid: &'a [f64],
    pub spectrum: &'a [Complex64],
    pub broadening: f64,
}

impl<'a> LorentzianConvolutionInput<'a> {
    pub fn new(energy_grid: &'a [f64], spectrum: &'a [Complex64], broadening: f64) -> Self {
        Self {
            energy_grid,
            spectrum,
            broadening,
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ConvolutionError {
    #[error("spectral helpers require at least 2 energy points, got {actual}")]
    InsufficientPoints { actual: usize },
    #[error("spectral input length mismatch: energy={energy}, spectrum={spectrum}")]
    LengthMismatch { energy: usize, spectrum: usize },
    #[error("energy grid entry must be finite at index {index}, got {value}")]
    NonFiniteEnergy { index: usize, value: f64 },
    #[error(
        "energy grid must be strictly increasing, index {index} has {current} after {previous}"
    )]
    NonIncreasingEnergy {
        index: usize,
        previous: f64,
        current: f64,
    },
    #[error("spectrum value must be finite at index {index}, got {value}")]
    NonFiniteSpectrum { index: usize, value: Complex64 },
    #[error("interpolation query must be finite, got {value}")]
    NonFiniteInterpolationQuery { value: f64 },
    #[error("lorentzian broadening must be finite and > 0, got {value}")]
    InvalidBroadening { value: f64 },
}

pub trait SpectralConvolutionApi {
    fn interpolate_spectrum_linear(
        &self,
        input: SpectralInterpolationInput<'_>,
    ) -> Result<Complex64, ConvolutionError>;

    fn convolve_lorentzian(
        &self,
        input: LorentzianConvolutionInput<'_>,
    ) -> Result<Vec<Complex64>, ConvolutionError>;
}

/// FEFF-style linear interpolation with boundary clamping:
/// return the first/last spectrum value when querying outside the energy grid.
pub fn interpolate_spectrum_linear(
    input: SpectralInterpolationInput<'_>,
) -> Result<Complex64, ConvolutionError> {
    validate_energy_grid_and_spectrum(input.energy_grid, input.spectrum)?;

    if !input.energy.is_finite() {
        return Err(ConvolutionError::NonFiniteInterpolationQuery {
            value: input.energy,
        });
    }

    let energy_grid = input.energy_grid;
    let spectrum = input.spectrum;
    let last = energy_grid.len() - 1;

    if input.energy <= energy_grid[0] {
        return Ok(spectrum[0]);
    }
    if input.energy >= energy_grid[last] {
        return Ok(spectrum[last]);
    }

    match energy_grid.binary_search_by(|probe| probe.total_cmp(&input.energy)) {
        Ok(index) => Ok(spectrum[index]),
        Err(upper) => {
            let lower = upper - 1;
            let x0 = energy_grid[lower];
            let x1 = energy_grid[upper];
            let y0 = spectrum[lower];
            let y1 = spectrum[upper];
            let fraction = (input.energy - x0) / (x1 - x0);
            Ok(y0 + (y1 - y0) * fraction)
        }
    }
}

/// FEFF `conv.f90` parity port:
/// convolute `spectrum(omega)` with `broadening / ((omega - omega0)^2 + broadening^2) / pi`.
pub fn convolve_lorentzian(
    input: LorentzianConvolutionInput<'_>,
) -> Result<Vec<Complex64>, ConvolutionError> {
    validate_energy_grid_and_spectrum(input.energy_grid, input.spectrum)?;
    if !input.broadening.is_finite() || input.broadening <= 0.0 {
        return Err(ConvolutionError::InvalidBroadening {
            value: input.broadening,
        });
    }

    let point_count = input.energy_grid.len();
    let last = point_count - 1;
    let last_step = input.energy_grid[last] - input.energy_grid[last - 1];
    let high_energy_extension = last_step.max(HIGH_ENERGY_EXTENSION_FACTOR * input.broadening);
    let x_last = input.energy_grid[last] + high_energy_extension;
    let extrapolation_scale = high_energy_extension / last_step;
    let y_last = input.spectrum[last]
        + (input.spectrum[last] - input.spectrum[last - 1]) * extrapolation_scale;

    let mut output = vec![Complex64::new(0.0, 0.0); point_count];
    for (target_index, target_energy) in input.energy_grid.iter().copied().enumerate() {
        let mut convolved = Complex64::new(0.0, 0.0);

        for interval in 0..last {
            convolved += conv_interval(
                input.energy_grid[interval],
                input.energy_grid[interval + 1],
                input.spectrum[interval],
                input.spectrum[interval + 1],
                target_energy,
                input.broadening,
            );
        }

        convolved += conv_interval(
            input.energy_grid[last],
            x_last,
            input.spectrum[last],
            y_last,
            target_energy,
            input.broadening,
        );

        output[target_index] = convolved / PI;
    }

    Ok(output)
}

fn validate_energy_grid_and_spectrum(
    energy_grid: &[f64],
    spectrum: &[Complex64],
) -> Result<(), ConvolutionError> {
    if energy_grid.len() < 2 {
        return Err(ConvolutionError::InsufficientPoints {
            actual: energy_grid.len(),
        });
    }
    if energy_grid.len() != spectrum.len() {
        return Err(ConvolutionError::LengthMismatch {
            energy: energy_grid.len(),
            spectrum: spectrum.len(),
        });
    }

    for (index, value) in energy_grid.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(ConvolutionError::NonFiniteEnergy { index, value });
        }

        if index > 0 {
            let previous = energy_grid[index - 1];
            if value <= previous {
                return Err(ConvolutionError::NonIncreasingEnergy {
                    index,
                    previous,
                    current: value,
                });
            }
        }
    }

    for (index, value) in spectrum.iter().copied().enumerate() {
        if !value.re.is_finite() || !value.im.is_finite() {
            return Err(ConvolutionError::NonFiniteSpectrum { index, value });
        }
    }

    Ok(())
}

fn conv_interval(
    x1: f64,
    x2: f64,
    y1: Complex64,
    y2: Complex64,
    x0: f64,
    broadening: f64,
) -> Complex64 {
    let real = conv_interval_scalar(x1, x2, y1.re, y2.re, x0, broadening);
    let imag = conv_interval_scalar(x1, x2, y1.im, y2.im, x0, broadening);
    Complex64::new(real, imag)
}

fn conv_interval_scalar(x1: f64, x2: f64, y1: f64, y2: f64, x0: f64, broadening: f64) -> f64 {
    let half_width = (x2 - x1) * 0.5;
    let a = (y2 - y1) * 0.5;
    let b = (y2 + y1) * 0.5;
    let center = (x1 + x2) * 0.5;
    let t = Complex64::new(half_width, 0.0) / Complex64::new(center - x0, -broadening);

    let one = Complex64::new(1.0, 0.0);
    let dum = if t.norm() >= SMALL_T_CUTOFF {
        Complex64::new(2.0 * a, 0.0)
            + (Complex64::new(b, 0.0) - Complex64::new(a, 0.0) / t) * ((one + t) / (one - t)).ln()
    } else {
        Complex64::new(2.0 * b, 0.0) * (t + t.powu(3) / 3.0)
            - Complex64::new((2.0 / 3.0) * a, 0.0) * t * t
    };

    dum.im
}

#[cfg(test)]
mod tests {
    use super::{
        convolve_lorentzian, interpolate_spectrum_linear, ConvolutionError,
        LorentzianConvolutionInput, SpectralInterpolationInput,
    };
    use num_complex::Complex64;
    use std::f64::consts::PI;

    const NUMERIC_SAMPLES_PER_INTERVAL: usize = 8_192;

    #[test]
    fn interpolation_clamps_and_interpolates_complex_spectra() {
        let energy_grid = [0.0, 1.0, 3.0];
        let spectrum = [
            Complex64::new(1.0, 2.0),
            Complex64::new(3.0, 4.0),
            Complex64::new(7.0, 10.0),
        ];

        let below = interpolate_spectrum_linear(SpectralInterpolationInput::new(
            -0.5,
            &energy_grid,
            &spectrum,
        ))
        .expect("lower clamp");
        let above = interpolate_spectrum_linear(SpectralInterpolationInput::new(
            4.0,
            &energy_grid,
            &spectrum,
        ))
        .expect("upper clamp");
        let interior = interpolate_spectrum_linear(SpectralInterpolationInput::new(
            2.0,
            &energy_grid,
            &spectrum,
        ))
        .expect("interior interpolation");

        assert_eq!(below, spectrum[0]);
        assert_eq!(above, spectrum[2]);
        assert_complex_close(
            "interior interpolation",
            Complex64::new(5.0, 7.0),
            interior,
            1.0e-15,
            1.0e-14,
        );
    }

    #[test]
    fn interpolation_rejects_invalid_inputs() {
        let energy_grid = [0.0, 1.0, 0.5];
        let spectrum = [
            Complex64::new(1.0, 0.0),
            Complex64::new(2.0, 0.0),
            Complex64::new(3.0, 0.0),
        ];

        let error = interpolate_spectrum_linear(SpectralInterpolationInput::new(
            0.2,
            &energy_grid,
            &spectrum,
        ))
        .expect_err("non-increasing grid should fail");
        assert_eq!(
            error,
            ConvolutionError::NonIncreasingEnergy {
                index: 2,
                previous: 1.0,
                current: 0.5,
            }
        );

        let error = interpolate_spectrum_linear(SpectralInterpolationInput::new(
            f64::NAN,
            &[0.0, 1.0],
            &[Complex64::new(0.0, 0.0), Complex64::new(1.0, 1.0)],
        ))
        .expect_err("non-finite interpolation query should fail");
        match error {
            ConvolutionError::NonFiniteInterpolationQuery { value } => {
                assert!(value.is_nan(), "expected NaN query value, got {value}")
            }
            other => panic!("expected NonFiniteInterpolationQuery, got {other:?}"),
        }
    }

    #[test]
    fn convolution_matches_independent_quadrature_for_representative_spectrum() {
        let energy_grid = [0.0, 0.4, 1.1, 1.7, 2.8];
        let spectrum = [
            Complex64::new(0.2, -0.4),
            Complex64::new(0.8, 0.3),
            Complex64::new(1.6, 0.9),
            Complex64::new(0.6, 1.4),
            Complex64::new(-0.1, 1.9),
        ];
        let broadening = 0.12;

        let actual = convolve_lorentzian(LorentzianConvolutionInput::new(
            &energy_grid,
            &spectrum,
            broadening,
        ))
        .expect("convolution");
        let expected = brute_force_convolution(
            &energy_grid,
            &spectrum,
            broadening,
            true,
            NUMERIC_SAMPLES_PER_INTERVAL,
        );

        for (index, (expected_value, actual_value)) in expected.iter().zip(&actual).enumerate() {
            assert_complex_close(
                &format!("representative spectrum index {index}"),
                *expected_value,
                *actual_value,
                5.0e-6,
                5.0e-6,
            );
        }
    }

    #[test]
    fn convolution_uses_high_energy_boundary_extension() {
        let energy_grid = [0.0, 1.0, 2.0];
        let spectrum = [
            Complex64::new(0.0, 0.0),
            Complex64::new(0.0, 0.0),
            Complex64::new(10.0, 4.0),
        ];
        let broadening = 0.05;

        let actual = convolve_lorentzian(LorentzianConvolutionInput::new(
            &energy_grid,
            &spectrum,
            broadening,
        ))
        .expect("convolution");
        let expected_with_extension = brute_force_convolution(
            &energy_grid,
            &spectrum,
            broadening,
            true,
            NUMERIC_SAMPLES_PER_INTERVAL,
        );
        let expected_without_extension = brute_force_convolution(
            &energy_grid,
            &spectrum,
            broadening,
            false,
            NUMERIC_SAMPLES_PER_INTERVAL,
        );

        assert_complex_close(
            "extension parity",
            expected_with_extension[2],
            actual[2],
            5.0e-6,
            5.0e-6,
        );

        let no_extension_delta = (actual[2] - expected_without_extension[2]).norm();
        assert!(
            no_extension_delta > 1.0e-2,
            "expected extension-sensitive tail contribution, got delta={no_extension_delta:.15e}"
        );
    }

    #[test]
    fn convolution_rejects_invalid_inputs() {
        let error = convolve_lorentzian(LorentzianConvolutionInput::new(
            &[0.0, 1.0],
            &[Complex64::new(1.0, 0.0)],
            0.1,
        ))
        .expect_err("mismatch should fail");
        assert_eq!(
            error,
            ConvolutionError::LengthMismatch {
                energy: 2,
                spectrum: 1,
            }
        );

        let error = convolve_lorentzian(LorentzianConvolutionInput::new(
            &[0.0, 0.0],
            &[Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
            0.1,
        ))
        .expect_err("duplicate energy should fail");
        assert_eq!(
            error,
            ConvolutionError::NonIncreasingEnergy {
                index: 1,
                previous: 0.0,
                current: 0.0,
            }
        );

        let error = convolve_lorentzian(LorentzianConvolutionInput::new(
            &[0.0, 1.0],
            &[Complex64::new(1.0, 0.0), Complex64::new(2.0, 0.0)],
            0.0,
        ))
        .expect_err("non-positive broadening should fail");
        assert_eq!(error, ConvolutionError::InvalidBroadening { value: 0.0 });
    }

    fn brute_force_convolution(
        energy_grid: &[f64],
        spectrum: &[Complex64],
        broadening: f64,
        include_extension: bool,
        samples_per_interval: usize,
    ) -> Vec<Complex64> {
        let segments = piecewise_segments(energy_grid, spectrum, broadening, include_extension);

        energy_grid
            .iter()
            .copied()
            .map(|target_energy| {
                let mut value = Complex64::new(0.0, 0.0);
                for (x1, x2, y1, y2) in &segments {
                    value += integrate_interval_numerically(
                        *x1,
                        *x2,
                        *y1,
                        *y2,
                        target_energy,
                        broadening,
                        samples_per_interval,
                    );
                }
                value / PI
            })
            .collect()
    }

    fn piecewise_segments(
        energy_grid: &[f64],
        spectrum: &[Complex64],
        broadening: f64,
        include_extension: bool,
    ) -> Vec<(f64, f64, Complex64, Complex64)> {
        let mut segments = Vec::with_capacity(energy_grid.len());
        for index in 0..(energy_grid.len() - 1) {
            segments.push((
                energy_grid[index],
                energy_grid[index + 1],
                spectrum[index],
                spectrum[index + 1],
            ));
        }

        if include_extension {
            let last = energy_grid.len() - 1;
            let last_step = energy_grid[last] - energy_grid[last - 1];
            let extension = last_step.max(super::HIGH_ENERGY_EXTENSION_FACTOR * broadening);
            let x_last = energy_grid[last] + extension;
            let scale = extension / last_step;
            let y_last = spectrum[last] + (spectrum[last] - spectrum[last - 1]) * scale;
            segments.push((energy_grid[last], x_last, spectrum[last], y_last));
        }

        segments
    }

    fn integrate_interval_numerically(
        x1: f64,
        x2: f64,
        y1: Complex64,
        y2: Complex64,
        target_energy: f64,
        broadening: f64,
        samples: usize,
    ) -> Complex64 {
        let samples = if samples.is_multiple_of(2) {
            samples
        } else {
            samples + 1
        };
        let step = (x2 - x1) / samples as f64;

        let mut odd_sum = Complex64::new(0.0, 0.0);
        let mut even_sum = Complex64::new(0.0, 0.0);
        for sample in 1..samples {
            let x = x1 + step * sample as f64;
            let value = linear_value(x, x1, x2, y1, y2);
            let kernel = broadening / ((x - target_energy).powi(2) + broadening * broadening);
            let integrand = value * kernel;
            if sample % 2 == 0 {
                even_sum += integrand;
            } else {
                odd_sum += integrand;
            }
        }

        let first = linear_value(x1, x1, x2, y1, y2)
            * (broadening / ((x1 - target_energy).powi(2) + broadening * broadening));
        let last = linear_value(x2, x1, x2, y1, y2)
            * (broadening / ((x2 - target_energy).powi(2) + broadening * broadening));
        (first + last + odd_sum * 4.0 + even_sum * 2.0) * (step / 3.0)
    }

    fn linear_value(x: f64, x1: f64, x2: f64, y1: Complex64, y2: Complex64) -> Complex64 {
        let t = (x - x1) / (x2 - x1);
        y1 + (y2 - y1) * t
    }

    fn assert_complex_close(
        label: &str,
        expected: Complex64,
        actual: Complex64,
        abs_tol: f64,
        rel_tol: f64,
    ) {
        let delta = actual - expected;
        let abs_diff = delta.norm();
        let rel_diff = abs_diff / expected.norm().max(1.0);
        assert!(
            abs_diff <= abs_tol || rel_diff <= rel_tol,
            "{label} expected=({:.15e},{:.15e}) actual=({:.15e},{:.15e}) abs_diff={:.15e} rel_diff={:.15e} abs_tol={:.15e} rel_tol={:.15e}",
            expected.re,
            expected.im,
            actual.re,
            actual.im,
            abs_diff,
            rel_diff,
            abs_tol,
            rel_tol
        );
    }
}
