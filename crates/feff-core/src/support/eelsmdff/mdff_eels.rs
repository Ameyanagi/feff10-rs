use super::mdff_m_spectrum::MdffSpectrum;
use num_complex::Complex64;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SigmaTensorRow {
    pub energy_loss_ev: f64,
    pub tensor: [[f64; 3]; 3],
}

impl SigmaTensorRow {
    pub fn from_flat(energy_loss_ev: f64, values: [f64; 9]) -> Self {
        Self {
            energy_loss_ev,
            tensor: [
                [values[0], values[1], values[2]],
                [values[3], values[4], values[5]],
                [values[6], values[7], values[8]],
            ],
        }
    }

    pub fn flatten(self) -> [f64; 9] {
        [
            self.tensor[0][0],
            self.tensor[0][1],
            self.tensor[0][2],
            self.tensor[1][0],
            self.tensor[1][1],
            self.tensor[1][2],
            self.tensor[2][0],
            self.tensor[2][1],
            self.tensor[2][2],
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnergyQMesh {
    pub q_vectors: Vec<[f64; 3]>,
    pub q_lengths_classical: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MdffEelsConfig {
    pub relativistic_q: bool,
    pub hbarc_ev: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum MdffScalingError {
    #[error("beam energy must be finite and positive, got {value}")]
    InvalidBeamEnergy { value: f64 },
    #[error("hbarc_atomic must be finite and positive, got {value}")]
    InvalidHbarcAtomic { value: f64 },
    #[error("me_c2_ev must be finite and positive, got {value}")]
    InvalidMeC2 { value: f64 },
    #[error("energy loss at index {index} must be finite and positive, got {value}")]
    InvalidEnergyLoss { index: usize, value: f64 },
    #[error("wavelength is non-finite or non-positive at index {index}")]
    InvalidWavelength { index: usize },
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum MdffEelsError {
    #[error("beam amplitude list cannot be empty")]
    EmptyBeamAmplitudes,
    #[error("sigma row count ({sigma_rows}) must match q-mesh row count ({q_mesh_rows})")]
    EnergyGridMismatch {
        sigma_rows: usize,
        q_mesh_rows: usize,
    },
    #[error("hbarc_ev must be finite and positive")]
    InvalidHbarc,
    #[error("q-mesh row {index} has {q_vectors} vectors but {q_lengths} classical lengths")]
    QMeshShapeMismatch {
        index: usize,
        q_vectors: usize,
        q_lengths: usize,
    },
    #[error(
        "q-mesh row {index} expected {expected} channels from beam amplitudes but found {actual}"
    )]
    QMeshChannelMismatch {
        index: usize,
        expected: usize,
        actual: usize,
    },
    #[error(
        "q-factor denominator is singular at energy index {energy_index}, channel pair ({iq}, {iqq})"
    )]
    SingularDenominator {
        energy_index: usize,
        iq: usize,
        iqq: usize,
    },
}

pub fn normalize_wave_amplitudes(amplitudes: &mut [Complex64]) {
    for amplitude in amplitudes {
        let magnitude = amplitude.norm();
        if magnitude > 0.0 {
            *amplitude /= magnitude;
        }
    }
}

pub fn scale_sigma_rows_with_wavelength<F>(
    sigma_rows: &mut [SigmaTensorRow],
    beam_energy_ev: f64,
    hbarc_atomic: f64,
    me_c2_ev: f64,
    wavelength: F,
) -> Result<(), MdffScalingError>
where
    F: Fn(f64) -> f64,
{
    if !beam_energy_ev.is_finite() || beam_energy_ev <= 0.0 {
        return Err(MdffScalingError::InvalidBeamEnergy {
            value: beam_energy_ev,
        });
    }
    if !hbarc_atomic.is_finite() || hbarc_atomic <= 0.0 {
        return Err(MdffScalingError::InvalidHbarcAtomic {
            value: hbarc_atomic,
        });
    }
    if !me_c2_ev.is_finite() || me_c2_ev <= 0.0 {
        return Err(MdffScalingError::InvalidMeC2 { value: me_c2_ev });
    }

    let wave_initial = wavelength(beam_energy_ev);
    if !wave_initial.is_finite() || wave_initial <= 0.0 {
        return Err(MdffScalingError::InvalidWavelength { index: 0 });
    }

    let gamma = 1.0 + beam_energy_ev / me_c2_ev;
    let gamma_sq = gamma * gamma;

    for (index, row) in sigma_rows.iter_mut().enumerate() {
        if !row.energy_loss_ev.is_finite() || row.energy_loss_ev <= 0.0 {
            return Err(MdffScalingError::InvalidEnergyLoss {
                index,
                value: row.energy_loss_ev,
            });
        }

        let wave_final = wavelength(beam_energy_ev - row.energy_loss_ev);
        if !wave_final.is_finite() || wave_final <= 0.0 {
            return Err(MdffScalingError::InvalidWavelength { index: index + 1 });
        }

        let factor =
            (wave_initial / wave_final) * gamma_sq / PI * hbarc_atomic / row.energy_loss_ev;
        for component_row in &mut row.tensor {
            for component in component_row {
                *component *= factor;
            }
        }
    }

    Ok(())
}

pub fn mdff_eels(
    sigma_rows: &[SigmaTensorRow],
    q_mesh_rows: &[EnergyQMesh],
    amplitudes: &[Complex64],
    config: MdffEelsConfig,
) -> Result<MdffSpectrum, MdffEelsError> {
    if amplitudes.is_empty() {
        return Err(MdffEelsError::EmptyBeamAmplitudes);
    }
    if sigma_rows.len() != q_mesh_rows.len() {
        return Err(MdffEelsError::EnergyGridMismatch {
            sigma_rows: sigma_rows.len(),
            q_mesh_rows: q_mesh_rows.len(),
        });
    }
    if !config.hbarc_ev.is_finite() || config.hbarc_ev <= 0.0 {
        return Err(MdffEelsError::InvalidHbarc);
    }

    let nq = amplitudes.len();
    let ne = sigma_rows.len();

    let mut output = MdffSpectrum::default();
    output.allocate_spectrum_1(ne, 9);
    output.allocate_spectrum_2(ne, 9, nq);

    for (energy_index, sigma_row) in sigma_rows.iter().enumerate() {
        output.s[energy_index][0] = sigma_row.energy_loss_ev;
        for (component_index, value) in sigma_row.flatten().into_iter().enumerate() {
            output.s[energy_index][component_index + 1] = value;
        }

        let q_mesh_row = &q_mesh_rows[energy_index];
        if q_mesh_row.q_vectors.len() != q_mesh_row.q_lengths_classical.len() {
            return Err(MdffEelsError::QMeshShapeMismatch {
                index: energy_index,
                q_vectors: q_mesh_row.q_vectors.len(),
                q_lengths: q_mesh_row.q_lengths_classical.len(),
            });
        }
        if q_mesh_row.q_vectors.len() != nq {
            return Err(MdffEelsError::QMeshChannelMismatch {
                index: energy_index,
                expected: nq,
                actual: q_mesh_row.q_vectors.len(),
            });
        }

        for iq in 0..nq {
            for iqq in 0..nq {
                let qfac = q_factor(
                    q_mesh_row.q_lengths_classical[iq],
                    sigma_row.energy_loss_ev,
                    config.hbarc_ev,
                    config.relativistic_q,
                );
                let qqfac = q_factor(
                    q_mesh_row.q_lengths_classical[iqq],
                    sigma_row.energy_loss_ev,
                    config.hbarc_ev,
                    config.relativistic_q,
                );

                let denominator = qfac * qqfac;
                if !denominator.is_finite() || denominator.abs() <= 1.0e-30 {
                    return Err(MdffEelsError::SingularDenominator {
                        energy_index,
                        iq,
                        iqq,
                    });
                }

                let prefactor = amplitudes[iq] * amplitudes[iqq].conj() / denominator;
                let channel_index = iqq + iq * nq + 1;

                for j1 in 0..3 {
                    for j2 in 0..3 {
                        let tensor_index = j1 * 3 + j2;
                        let term = prefactor
                            * q_mesh_row.q_vectors[iq][j1]
                            * q_mesh_row.q_vectors[iqq][j2]
                            * sigma_row.tensor[j1][j2];

                        output.x[energy_index][0] += term;
                        output.x[energy_index][channel_index] += term;
                        output.xpart[energy_index][tensor_index][0] += term;
                        output.xpart[energy_index][tensor_index][channel_index] += term;
                    }
                }
            }
        }
    }

    Ok(output)
}

fn q_factor(
    q_length_classical: f64,
    energy_loss_ev: f64,
    hbarc_ev: f64,
    relativistic: bool,
) -> f64 {
    let q2 = q_length_classical * q_length_classical;
    if relativistic {
        q2 - (energy_loss_ev / hbarc_ev).powi(2)
    } else {
        q2
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EnergyQMesh, MdffEelsConfig, MdffEelsError, SigmaTensorRow, mdff_eels,
        normalize_wave_amplitudes, scale_sigma_rows_with_wavelength,
    };
    use num_complex::Complex64;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn flatten_round_trip_preserves_tensor_layout() {
        let row = SigmaTensorRow::from_flat(1.0, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
        assert_eq!(row.flatten(), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    }

    #[test]
    fn normalize_wave_amplitudes_preserves_phase() {
        let mut amplitudes = vec![Complex64::new(0.8, -0.2), Complex64::new(0.0, 0.0)];
        normalize_wave_amplitudes(&mut amplitudes);

        assert_close(amplitudes[0].norm(), 1.0, 1.0e-12);
        assert_eq!(amplitudes[1], Complex64::new(0.0, 0.0));
    }

    #[test]
    fn scale_sigma_rows_applies_prefactor() {
        let mut rows = vec![SigmaTensorRow {
            energy_loss_ev: 200.0,
            tensor: [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 5.0]],
        }];

        scale_sigma_rows_with_wavelength(&mut rows, 1_000.0, 10.0, 511_004.0, |energy| {
            energy.abs().sqrt()
        })
        .expect("scaling should succeed");

        let gamma = 1.0 + 1_000.0 / 511_004.0;
        let expected_factor =
            (1_000.0_f64.sqrt() / 800.0_f64.sqrt()) * gamma * gamma / std::f64::consts::PI * 10.0
                / 200.0;

        assert_close(rows[0].tensor[0][0], 2.0 * expected_factor, 1.0e-12);
        assert_close(rows[0].tensor[1][1], 3.0 * expected_factor, 1.0e-12);
        assert_close(rows[0].tensor[2][2], 5.0 * expected_factor, 1.0e-12);
    }

    #[test]
    fn mdff_eels_accumulates_single_channel_cross_section() {
        let sigma_rows = vec![SigmaTensorRow {
            energy_loss_ev: 2.0,
            tensor: [[2.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
        }];
        let q_rows = vec![EnergyQMesh {
            q_vectors: vec![[1.0, 0.0, 0.0]],
            q_lengths_classical: vec![1.0],
        }];
        let amplitudes = vec![Complex64::new(1.0, 0.0)];

        let spectrum = mdff_eels(
            &sigma_rows,
            &q_rows,
            &amplitudes,
            MdffEelsConfig {
                relativistic_q: false,
                hbarc_ev: 10.0,
            },
        )
        .expect("mdff loop should succeed");

        assert_eq!(spectrum.ne, 1);
        assert_eq!(spectrum.x.len(), 1);
        assert_eq!(spectrum.x[0].len(), 2);

        assert_close(spectrum.x[0][0].re, 2.0, 1.0e-12);
        assert_close(spectrum.x[0][1].re, 2.0, 1.0e-12);
        assert_close(spectrum.xpart[0][0][0].re, 2.0, 1.0e-12);
        assert_close(spectrum.xpart[0][0][1].re, 2.0, 1.0e-12);
    }

    #[test]
    fn relativistic_q_toggle_changes_denominator_weight() {
        let sigma_rows = vec![SigmaTensorRow {
            energy_loss_ev: 2.0,
            tensor: [[2.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
        }];
        let q_rows = vec![EnergyQMesh {
            q_vectors: vec![[1.0, 0.0, 0.0]],
            q_lengths_classical: vec![1.0],
        }];
        let amplitudes = vec![Complex64::new(1.0, 0.0)];

        let non_rel = mdff_eels(
            &sigma_rows,
            &q_rows,
            &amplitudes,
            MdffEelsConfig {
                relativistic_q: false,
                hbarc_ev: 4.0,
            },
        )
        .expect("non-relativistic run should succeed");

        let rel = mdff_eels(
            &sigma_rows,
            &q_rows,
            &amplitudes,
            MdffEelsConfig {
                relativistic_q: true,
                hbarc_ev: 4.0,
            },
        )
        .expect("relativistic run should succeed");

        assert!(rel.x[0][0].re > non_rel.x[0][0].re);
    }

    #[test]
    fn reports_q_mesh_channel_mismatch() {
        let error = mdff_eels(
            &[SigmaTensorRow {
                energy_loss_ev: 1.0,
                tensor: [[1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
            }],
            &[EnergyQMesh {
                q_vectors: vec![[1.0, 0.0, 0.0]],
                q_lengths_classical: vec![1.0],
            }],
            &[Complex64::new(1.0, 0.0), Complex64::new(0.0, 1.0)],
            MdffEelsConfig {
                relativistic_q: false,
                hbarc_ev: 10.0,
            },
        )
        .expect_err("mismatched channel counts should fail");

        assert_eq!(
            error,
            MdffEelsError::QMeshChannelMismatch {
                index: 0,
                expected: 2,
                actual: 1,
            }
        );
    }
}
