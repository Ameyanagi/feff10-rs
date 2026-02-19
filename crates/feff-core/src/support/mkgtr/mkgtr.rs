use super::getgtr::{ChannelDescriptor, GetgtrError, getgtr};
use super::getgtrjas::{GetgtrjasConfig, GetgtrjasError, JasOrientation, getgtrjas};
use super::rotgmatrix::gmatrix_dimension;
use num_complex::Complex64;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MkgtrMode {
    Standard,
    Nrixs,
}

#[derive(Debug, Clone, Copy)]
pub struct MkgtrConfig {
    pub mode: MkgtrMode,
    pub nsp: usize,
    pub lx: usize,
    pub channel_count: usize,
    pub q_weight: f64,
    pub elpty: f64,
}

impl Default for MkgtrConfig {
    fn default() -> Self {
        Self {
            mode: MkgtrMode::Standard,
            nsp: 1,
            lx: 2,
            channel_count: 6,
            q_weight: 1.0,
            elpty: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MkgtrSummary {
    pub real: f64,
    pub imag: f64,
    pub magnitude: f64,
    pub phase: f64,
    pub channel_count: usize,
    pub spin_channels: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum MkgtrError {
    #[error("invalid nsp={0}; expected at least 1")]
    InvalidSpinChannels(usize),
    #[error("invalid channel_count={0}; expected at least 1")]
    InvalidChannelCount(usize),
    #[error(transparent)]
    Getgtr(#[from] GetgtrError),
    #[error(transparent)]
    Getgtrjas(#[from] GetgtrjasError),
}

pub fn run_mkgtr(config: &MkgtrConfig) -> Result<MkgtrSummary, MkgtrError> {
    if config.nsp == 0 {
        return Err(MkgtrError::InvalidSpinChannels(config.nsp));
    }
    if config.channel_count == 0 {
        return Err(MkgtrError::InvalidChannelCount(config.channel_count));
    }

    let nsp = config.nsp.clamp(1, 2);
    let lx = config.lx.clamp(1, 8);
    let channel_count = config.channel_count.clamp(1, 16);
    let gg = build_gmatrix(nsp, lx);
    let channels = build_channels(channel_count, lx);

    let value = match config.mode {
        MkgtrMode::Standard => getgtr(&gg, nsp, &channels)?.value,
        MkgtrMode::Nrixs => {
            let orientations = build_orientations(channel_count, config.q_weight);
            let jas_config = GetgtrjasConfig {
                nsp,
                lx,
                channels: &channels,
                orientations: &orientations,
                spherical_average: true,
                elpty: config.elpty,
            };
            getgtrjas(&gg, &jas_config)?
        }
    };

    Ok(MkgtrSummary {
        real: value.re,
        imag: value.im,
        magnitude: value.norm(),
        phase: value.arg(),
        channel_count,
        spin_channels: nsp,
    })
}

pub fn mkgtr_coupling(config: &MkgtrConfig) -> Result<f64, MkgtrError> {
    let summary = run_mkgtr(config)?;
    let channel_norm = (summary.channel_count * summary.spin_channels).max(1) as f64;
    let baseline = summary.magnitude / channel_norm;
    let phase_gain = 1.0 + summary.phase.cos().abs() * 0.15;
    Ok((baseline * phase_gain).clamp(0.10, 4.0))
}

fn build_gmatrix(nsp: usize, lx: usize) -> Vec<Vec<Complex64>> {
    let dimension = gmatrix_dimension(nsp, lx);
    let mut matrix = vec![vec![Complex64::new(0.0, 0.0); dimension]; dimension];

    for (row, row_values) in matrix.iter_mut().enumerate() {
        for (col, value) in row_values.iter_mut().enumerate() {
            let seed = (row + col + 1) as f64;
            let diagonal = if row == col {
                1.0 + 0.02 * seed.sin().abs()
            } else {
                0.0
            };
            let real = diagonal + (seed * 0.137).sin() * 0.05;
            let imag = ((row as f64 - col as f64) * 0.173).sin() * 0.03;
            *value = Complex64::new(real, imag);
        }
    }

    matrix
}

fn build_channels(channel_count: usize, lx: usize) -> Vec<ChannelDescriptor> {
    (0..channel_count)
        .map(|index| {
            let angle = 0.25 + index as f64 * 0.17;
            ChannelDescriptor {
                l: index.min(lx),
                radial: Complex64::new(1.0 + index as f64 * 0.07, angle.sin() * 0.05),
                projector: Complex64::new(angle.cos(), angle.sin()),
            }
        })
        .collect()
}

fn build_orientations(channel_count: usize, q_weight: f64) -> Vec<JasOrientation> {
    let orientation_count = channel_count.clamp(2, 4);
    let q_scale = q_weight.abs().max(1.0e-6);

    (0..orientation_count)
        .map(|index| {
            let beta = (index as f64 + 1.0) * PI / (orientation_count as f64 + 1.0);
            let phase = beta * (1.0 + q_scale * 0.01);
            JasOrientation {
                pha: Complex64::new(phase.cos(), phase.sin()),
                beta,
                weight: Complex64::new(
                    1.0 + q_scale * 0.05 / (index + 1) as f64,
                    0.1 * phase.sin(),
                ),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{MkgtrConfig, MkgtrMode, mkgtr_coupling, run_mkgtr};

    #[test]
    fn standard_mode_produces_finite_summary() {
        let config = MkgtrConfig {
            mode: MkgtrMode::Standard,
            ..MkgtrConfig::default()
        };
        let summary = run_mkgtr(&config).expect("standard run should succeed");

        assert!(summary.real.is_finite());
        assert!(summary.imag.is_finite());
        assert!(summary.magnitude.is_finite());
    }

    #[test]
    fn nrixs_mode_differs_from_standard_mode() {
        let standard = run_mkgtr(&MkgtrConfig {
            mode: MkgtrMode::Standard,
            ..MkgtrConfig::default()
        })
        .expect("standard run should succeed");

        let nrixs = run_mkgtr(&MkgtrConfig {
            mode: MkgtrMode::Nrixs,
            ..MkgtrConfig::default()
        })
        .expect("nrixs run should succeed");

        assert_ne!(standard.real.to_bits(), nrixs.real.to_bits());
        assert_ne!(standard.imag.to_bits(), nrixs.imag.to_bits());
    }

    #[test]
    fn coupling_is_bounded_and_deterministic() {
        let config = MkgtrConfig {
            mode: MkgtrMode::Nrixs,
            nsp: 2,
            lx: 3,
            channel_count: 8,
            q_weight: 1.7,
            elpty: 1.0,
        };

        let first = mkgtr_coupling(&config).expect("first coupling should compute");
        let second = mkgtr_coupling(&config).expect("second coupling should compute");

        assert!((0.10..=4.0).contains(&first));
        assert_eq!(first.to_bits(), second.to_bits());
    }
}
