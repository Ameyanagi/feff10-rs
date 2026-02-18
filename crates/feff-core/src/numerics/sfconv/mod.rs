use std::f64::consts::PI;

const MIN_NORMALIZATION: f64 = 1.0e-18;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SfconvConvolutionInput<'a> {
    pub photoelectron_energy: f64,
    pub chemical_potential: f64,
    pub core_hole_lifetime: f64,
    pub signal_energies: &'a [f64],
    pub signal_values: &'a [f64],
    pub spectral_energies: &'a [f64],
    pub spectral_values: &'a [f64],
    pub weights: [f64; 8],
    pub use_asymmetric_phase: bool,
    pub apply_energy_cutoff: bool,
    pub plasma_frequency: f64,
}

impl<'a> SfconvConvolutionInput<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        photoelectron_energy: f64,
        chemical_potential: f64,
        core_hole_lifetime: f64,
        signal_energies: &'a [f64],
        signal_values: &'a [f64],
        spectral_energies: &'a [f64],
        spectral_values: &'a [f64],
        weights: [f64; 8],
        use_asymmetric_phase: bool,
        apply_energy_cutoff: bool,
        plasma_frequency: f64,
    ) -> Self {
        Self {
            photoelectron_energy,
            chemical_potential,
            core_hole_lifetime,
            signal_energies,
            signal_values,
            spectral_energies,
            spectral_values,
            weights,
            use_asymmetric_phase,
            apply_energy_cutoff,
            plasma_frequency,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SfconvConvolutionResult {
    pub magnitude: f64,
    pub phase: f64,
    pub real: f64,
    pub imaginary: f64,
    pub normalization: f64,
    pub quasiparticle_weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SfconvGridConvolutionInput<'a> {
    pub photoelectron_energies: &'a [f64],
    pub chemical_potential: f64,
    pub core_hole_lifetime: f64,
    pub signal_energies: &'a [f64],
    pub signal_values: &'a [f64],
    pub spectral_energies: &'a [f64],
    pub spectral_values: &'a [f64],
    pub weights: [f64; 8],
    pub use_asymmetric_phase: bool,
    pub apply_energy_cutoff: bool,
    pub plasma_frequency: f64,
}

impl<'a> SfconvGridConvolutionInput<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        photoelectron_energies: &'a [f64],
        chemical_potential: f64,
        core_hole_lifetime: f64,
        signal_energies: &'a [f64],
        signal_values: &'a [f64],
        spectral_energies: &'a [f64],
        spectral_values: &'a [f64],
        weights: [f64; 8],
        use_asymmetric_phase: bool,
        apply_energy_cutoff: bool,
        plasma_frequency: f64,
    ) -> Self {
        Self {
            photoelectron_energies,
            chemical_potential,
            core_hole_lifetime,
            signal_energies,
            signal_values,
            spectral_energies,
            spectral_values,
            weights,
            use_asymmetric_phase,
            apply_energy_cutoff,
            plasma_frequency,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SfconvGridConvolutionResult {
    pub magnitudes: Vec<f64>,
    pub phases: Vec<f64>,
    pub real: Vec<f64>,
    pub imaginary: Vec<f64>,
}

#[derive(Debug, thiserror::Error)]
pub enum SfconvError {
    #[error("SFCONV photoelectron energy must be finite, got {value}")]
    InvalidPhotoelectronEnergy { value: f64 },
    #[error("SFCONV photoelectron energy grid must contain at least one point")]
    EmptyPhotoelectronEnergyGrid,
    #[error("SFCONV photoelectron energy[{index}] must be finite, got {value}")]
    InvalidPhotoelectronEnergyGridValue { index: usize, value: f64 },
    #[error("SFCONV chemical potential must be finite, got {value}")]
    InvalidChemicalPotential { value: f64 },
    #[error("SFCONV core-hole lifetime must be finite and non-negative, got {value}")]
    InvalidCoreHoleLifetime { value: f64 },
    #[error("SFCONV signal arrays must contain at least two points, got {count}")]
    SignalTooShort { count: usize },
    #[error("SFCONV signal length mismatch: energies={energies}, values={values}")]
    SignalLengthMismatch { energies: usize, values: usize },
    #[error("SFCONV spectral arrays must contain at least two points, got {count}")]
    SpectralTooShort { count: usize },
    #[error("SFCONV spectral length mismatch: energies={energies}, values={values}")]
    SpectralLengthMismatch { energies: usize, values: usize },
    #[error(
        "SFCONV signal energy grid must be strictly increasing at index {index}: {previous} -> {current}"
    )]
    NonMonotonicSignalGrid {
        index: usize,
        previous: f64,
        current: f64,
    },
    #[error(
        "SFCONV spectral energy grid must be strictly increasing at index {index}: {previous} -> {current}"
    )]
    NonMonotonicSpectralGrid {
        index: usize,
        previous: f64,
        current: f64,
    },
    #[error("SFCONV signal value at index {index} must be finite, got {value}")]
    InvalidSignalValue { index: usize, value: f64 },
    #[error("SFCONV spectral value at index {index} must be finite, got {value}")]
    InvalidSpectralValue { index: usize, value: f64 },
    #[error("SFCONV weight[{index}] must be finite, got {value}")]
    InvalidWeight { index: usize, value: f64 },
    #[error(
        "SFCONV asymmetric branch requires non-zero quasiparticle amplitude from weights[0..2], got ({re}, {im})"
    )]
    InvalidAsymmetricWeights { re: f64, im: f64 },
    #[error("SFCONV plasma frequency must be finite and positive, got {value}")]
    InvalidPlasmaFrequency { value: f64 },
    #[error("SFCONV normalization must be finite and non-zero after cutoff, got {value}")]
    InvalidNormalization { value: f64 },
}

pub trait SfconvKernelApi {
    fn convolve_point(
        &self,
        input: SfconvConvolutionInput<'_>,
    ) -> Result<SfconvConvolutionResult, SfconvError>;

    fn convolve_grid(
        &self,
        input: SfconvGridConvolutionInput<'_>,
    ) -> Result<SfconvGridConvolutionResult, SfconvError>;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SfconvKernel;

impl SfconvKernelApi for SfconvKernel {
    fn convolve_point(
        &self,
        input: SfconvConvolutionInput<'_>,
    ) -> Result<SfconvConvolutionResult, SfconvError> {
        convolve_sfconv_point(input)
    }

    fn convolve_grid(
        &self,
        input: SfconvGridConvolutionInput<'_>,
    ) -> Result<SfconvGridConvolutionResult, SfconvError> {
        convolve_sfconv_grid(input)
    }
}

pub fn convolve_sfconv_point(
    input: SfconvConvolutionInput<'_>,
) -> Result<SfconvConvolutionResult, SfconvError> {
    validate_point_input(input)?;
    let prepared = prepare_spectral_window(input)?;

    let mut real = 0.0_f64;
    let mut imaginary = 0.0_f64;
    for (index, (&w, &dw)) in input
        .spectral_energies
        .iter()
        .zip(prepared.spectral_steps.iter())
        .enumerate()
    {
        let shifted_energy = input.photoelectron_energy - w;
        let interpolated_signal = interpolate_signal(input, shifted_energy);

        if index > 0 && index + 1 < input.spectral_energies.len() {
            let w_prev = input.spectral_energies[index - 1];
            let w_next = input.spectral_energies[index + 1];
            let crosses_zero = (w + w_prev) * 0.5 < 0.0 && (w + w_next) * 0.5 >= 0.0;
            if crosses_zero {
                real += prepared.quasiparticle_weight * interpolated_signal;
            }
        }

        real += prepared.cutoff_spectral[index] * dw * interpolated_signal;
    }

    let stored = real;
    real = stored * prepared.phase_shift.cos() - imaginary * prepared.phase_shift.sin();
    imaginary = imaginary * prepared.phase_shift.cos() + stored * prepared.phase_shift.sin();
    real /= prepared.normalization;
    imaginary /= prepared.normalization;

    Ok(SfconvConvolutionResult {
        magnitude: (real * real + imaginary * imaginary).sqrt(),
        phase: imaginary.atan2(real),
        real,
        imaginary,
        normalization: prepared.normalization,
        quasiparticle_weight: prepared.quasiparticle_weight,
    })
}

pub fn convolve_sfconv_grid(
    input: SfconvGridConvolutionInput<'_>,
) -> Result<SfconvGridConvolutionResult, SfconvError> {
    if input.photoelectron_energies.is_empty() {
        return Err(SfconvError::EmptyPhotoelectronEnergyGrid);
    }
    for (index, value) in input.photoelectron_energies.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(SfconvError::InvalidPhotoelectronEnergyGridValue { index, value });
        }
    }

    let mut magnitudes = Vec::with_capacity(input.photoelectron_energies.len());
    let mut phases = Vec::with_capacity(input.photoelectron_energies.len());
    let mut real = Vec::with_capacity(input.photoelectron_energies.len());
    let mut imaginary = Vec::with_capacity(input.photoelectron_energies.len());

    for &photoelectron_energy in input.photoelectron_energies {
        let result = convolve_sfconv_point(SfconvConvolutionInput {
            photoelectron_energy,
            chemical_potential: input.chemical_potential,
            core_hole_lifetime: input.core_hole_lifetime,
            signal_energies: input.signal_energies,
            signal_values: input.signal_values,
            spectral_energies: input.spectral_energies,
            spectral_values: input.spectral_values,
            weights: input.weights,
            use_asymmetric_phase: input.use_asymmetric_phase,
            apply_energy_cutoff: input.apply_energy_cutoff,
            plasma_frequency: input.plasma_frequency,
        })?;

        magnitudes.push(result.magnitude);
        phases.push(result.phase);
        real.push(result.real);
        imaginary.push(result.imaginary);
    }

    Ok(SfconvGridConvolutionResult {
        magnitudes,
        phases,
        real,
        imaginary,
    })
}

#[derive(Debug)]
struct PreparedSpectralWindow {
    cutoff_spectral: Vec<f64>,
    spectral_steps: Vec<f64>,
    normalization: f64,
    quasiparticle_weight: f64,
    phase_shift: f64,
}

fn validate_point_input(input: SfconvConvolutionInput<'_>) -> Result<(), SfconvError> {
    if !input.photoelectron_energy.is_finite() {
        return Err(SfconvError::InvalidPhotoelectronEnergy {
            value: input.photoelectron_energy,
        });
    }
    if !input.chemical_potential.is_finite() {
        return Err(SfconvError::InvalidChemicalPotential {
            value: input.chemical_potential,
        });
    }
    if !input.core_hole_lifetime.is_finite() || input.core_hole_lifetime < 0.0 {
        return Err(SfconvError::InvalidCoreHoleLifetime {
            value: input.core_hole_lifetime,
        });
    }

    if input.signal_energies.len() != input.signal_values.len() {
        return Err(SfconvError::SignalLengthMismatch {
            energies: input.signal_energies.len(),
            values: input.signal_values.len(),
        });
    }
    if input.signal_energies.len() < 2 {
        return Err(SfconvError::SignalTooShort {
            count: input.signal_energies.len(),
        });
    }
    if input.spectral_energies.len() != input.spectral_values.len() {
        return Err(SfconvError::SpectralLengthMismatch {
            energies: input.spectral_energies.len(),
            values: input.spectral_values.len(),
        });
    }
    if input.spectral_energies.len() < 2 {
        return Err(SfconvError::SpectralTooShort {
            count: input.spectral_energies.len(),
        });
    }

    validate_grid(input.signal_energies, true)?;
    validate_grid(input.spectral_energies, false)?;

    for (index, value) in input.signal_values.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(SfconvError::InvalidSignalValue { index, value });
        }
    }
    for (index, value) in input.spectral_values.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(SfconvError::InvalidSpectralValue { index, value });
        }
    }
    for (index, value) in input.weights.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(SfconvError::InvalidWeight { index, value });
        }
    }

    if input.use_asymmetric_phase {
        if input.weights[0].abs() <= f64::EPSILON {
            return Err(SfconvError::InvalidAsymmetricWeights {
                re: input.weights[0],
                im: input.weights[1],
            });
        }
        if !input.plasma_frequency.is_finite() || input.plasma_frequency <= 0.0 {
            return Err(SfconvError::InvalidPlasmaFrequency {
                value: input.plasma_frequency,
            });
        }
    } else if !input.plasma_frequency.is_finite() {
        return Err(SfconvError::InvalidPlasmaFrequency {
            value: input.plasma_frequency,
        });
    }

    Ok(())
}

fn validate_grid(grid: &[f64], signal: bool) -> Result<(), SfconvError> {
    for (index, &value) in grid.iter().enumerate() {
        if !value.is_finite() {
            if signal {
                return Err(SfconvError::NonMonotonicSignalGrid {
                    index,
                    previous: value,
                    current: value,
                });
            }
            return Err(SfconvError::NonMonotonicSpectralGrid {
                index,
                previous: value,
                current: value,
            });
        }
    }

    for index in 1..grid.len() {
        let previous = grid[index - 1];
        let current = grid[index];
        if current <= previous {
            if signal {
                return Err(SfconvError::NonMonotonicSignalGrid {
                    index,
                    previous,
                    current,
                });
            }
            return Err(SfconvError::NonMonotonicSpectralGrid {
                index,
                previous,
                current,
            });
        }
    }

    Ok(())
}

fn prepare_spectral_window(
    input: SfconvConvolutionInput<'_>,
) -> Result<PreparedSpectralWindow, SfconvError> {
    let amplitude = if input.use_asymmetric_phase {
        input.weights[0]
    } else {
        (input.weights[0] * input.weights[0] + input.weights[1] * input.weights[1]).sqrt()
    };

    let phase_shift = if input.weights[0] != 0.0 && !input.use_asymmetric_phase {
        (input.weights[1] / input.weights[0]).atan()
    } else {
        0.0
    };

    let quasiparticle_reduction = if !input.apply_energy_cutoff {
        1.0
    } else if input.photoelectron_energy - input.chemical_potential != 0.0 {
        input
            .core_hole_lifetime
            .atan2(input.chemical_potential - input.photoelectron_energy)
            / PI
    } else {
        0.5
    };

    let quasiparticle_weight = quasiparticle_reduction * (amplitude + input.weights[2]);
    let mut normalization = quasiparticle_weight;
    let mut cutoff_spectral = Vec::with_capacity(input.spectral_values.len());
    let mut spectral_steps = Vec::with_capacity(input.spectral_values.len());

    for (index, &spectral_energy) in input.spectral_energies.iter().enumerate() {
        let dw = interval_width(input.spectral_energies, index);
        let remaining_energy = input.photoelectron_energy - spectral_energy;

        let cutoff_weight = if !input.apply_energy_cutoff {
            1.0
        } else if remaining_energy - input.chemical_potential != 0.0 {
            input
                .core_hole_lifetime
                .atan2(input.chemical_potential - remaining_energy)
                / PI
        } else {
            0.5
        };

        let mut cutoff_value = if !input.apply_energy_cutoff {
            input.spectral_values[index]
        } else if spectral_energy >= 0.0 {
            input.spectral_values[index] * cutoff_weight
        } else {
            (input.spectral_values[index] * cutoff_weight).max(0.0)
        };

        if input.use_asymmetric_phase {
            let top = (spectral_energy + 0.5 * dw).powi(2) + (3.0 * dw).powi(2);
            let bottom = (spectral_energy - 0.5 * dw).powi(2) + (3.0 * dw).powi(2);
            let correction = quasiparticle_reduction
                * (input.weights[1] / (PI * amplitude * dw))
                * (top / bottom).ln()
                * (-(spectral_energy / (2.0 * input.plasma_frequency)).powi(2)).exp()
                * 0.5;
            cutoff_value -= correction;
        }

        normalization += cutoff_value * dw;
        cutoff_spectral.push(cutoff_value);
        spectral_steps.push(dw);
    }

    if !normalization.is_finite() || normalization.abs() <= MIN_NORMALIZATION {
        return Err(SfconvError::InvalidNormalization {
            value: normalization,
        });
    }

    Ok(PreparedSpectralWindow {
        cutoff_spectral,
        spectral_steps,
        normalization,
        quasiparticle_weight,
        phase_shift,
    })
}

fn interval_width(grid: &[f64], index: usize) -> f64 {
    if index == 0 {
        grid[1] - grid[0]
    } else if index + 1 == grid.len() {
        grid[grid.len() - 1] - grid[grid.len() - 2]
    } else {
        0.5 * (grid[index + 1] - grid[index - 1])
    }
}

fn interpolate_signal(input: SfconvConvolutionInput<'_>, shifted_energy: f64) -> f64 {
    let energies = input.signal_energies;
    let values = input.signal_values;
    let last_index = energies.len() - 1;

    if shifted_energy > energies[last_index] {
        return values[last_index];
    }
    if shifted_energy <= energies[0] {
        let amplitude = values[0];
        let delta = input.chemical_potential - energies[0];
        let denominator = PI
            * amplitude.abs()
            * (delta * delta + input.core_hole_lifetime * input.core_hole_lifetime);
        let lambda = if denominator > 0.0 {
            delta * delta / denominator
        } else {
            0.0
        };
        return amplitude * (lambda * (shifted_energy - energies[0])).exp();
    }

    match energies.binary_search_by(|energy| {
        energy
            .partial_cmp(&shifted_energy)
            .expect("signal energies should be finite")
    }) {
        Ok(index) => values[index],
        Err(upper) => {
            let lower = upper.saturating_sub(1);
            let fraction = (shifted_energy - energies[lower]) / (energies[upper] - energies[lower]);
            values[lower] + (values[upper] - values[lower]) * fraction
        }
    }
}
