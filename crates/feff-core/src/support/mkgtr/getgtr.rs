use super::rotgmatrix::gmatrix_dimension;
use num_complex::Complex64;

#[derive(Debug, Clone, Copy)]
pub struct ChannelDescriptor {
    pub l: usize,
    pub radial: Complex64,
    pub projector: Complex64,
}

#[derive(Debug, Clone, Copy)]
pub struct GtrTrace {
    pub value: Complex64,
    pub channel_count: usize,
    pub spin_channels: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum GetgtrError {
    #[error("matrix must be square and non-empty")]
    EmptyMatrix,
    #[error("matrix is not square")]
    NonSquareMatrix,
    #[error("matrix is too small for l={required_l} and nsp={nsp}; expected at least {expected}")]
    InsufficientDimension {
        required_l: usize,
        nsp: usize,
        expected: usize,
    },
}

pub fn getgtr(
    gg: &[Vec<Complex64>],
    nsp: usize,
    channels: &[ChannelDescriptor],
) -> Result<GtrTrace, GetgtrError> {
    if gg.is_empty() {
        return Err(GetgtrError::EmptyMatrix);
    }
    if gg.iter().any(|row| row.len() != gg.len()) {
        return Err(GetgtrError::NonSquareMatrix);
    }

    if channels.is_empty() {
        return Ok(GtrTrace {
            value: Complex64::new(0.0, 0.0),
            channel_count: 0,
            spin_channels: nsp,
        });
    }

    let max_l = channels.iter().map(|channel| channel.l).max().unwrap_or(0);
    let expected = gmatrix_dimension(nsp.max(1), max_l);
    if gg.len() < expected {
        return Err(GetgtrError::InsufficientDimension {
            required_l: max_l,
            nsp,
            expected,
        });
    }

    let mut trace = Complex64::new(0.0, 0.0);

    for channel_left in channels {
        for channel_right in channels {
            let radial_weight = channel_left.radial * channel_right.radial.conj();
            let projector_weight = channel_right.projector * channel_left.projector.conj();

            for is_left in 0..nsp {
                for is_right in 0..nsp {
                    for m_left in -(channel_left.l as i32)..=(channel_left.l as i32) {
                        let idx_left = lm_spin_index(channel_left.l, m_left, is_left, nsp.max(1));
                        for m_right in -(channel_right.l as i32)..=(channel_right.l as i32) {
                            let idx_right =
                                lm_spin_index(channel_right.l, m_right, is_right, nsp.max(1));
                            let angular_weight = 1.0
                                / (1.0
                                    + (m_left - m_right).unsigned_abs() as f64
                                    + (channel_left.l as f64 - channel_right.l as f64).abs());

                            trace += gg[idx_right][idx_left]
                                * radial_weight
                                * projector_weight
                                * angular_weight;
                        }
                    }
                }
            }
        }
    }

    Ok(GtrTrace {
        value: trace,
        channel_count: channels.len(),
        spin_channels: nsp,
    })
}

fn lm_spin_index(l: usize, m: i32, spin: usize, nsp: usize) -> usize {
    let idx_1_based = (nsp as i32) * (l * l + l) as i32 + (nsp as i32) * m + (spin as i32 + 1);
    (idx_1_based - 1) as usize
}

#[cfg(test)]
mod tests {
    use super::{ChannelDescriptor, getgtr};
    use num_complex::Complex64;

    fn identity_matrix(size: usize) -> Vec<Vec<Complex64>> {
        let mut matrix = vec![vec![Complex64::new(0.0, 0.0); size]; size];
        for (index, row) in matrix.iter_mut().enumerate() {
            row[index] = Complex64::new(1.0, 0.0);
        }
        matrix
    }

    fn assert_close(actual: Complex64, expected: Complex64, tolerance: f64) {
        assert!(
            (actual - expected).norm() <= tolerance,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn single_channel_identity_matrix_has_unit_trace() {
        let gg = identity_matrix(1);
        let channels = vec![ChannelDescriptor {
            l: 0,
            radial: Complex64::new(1.0, 0.0),
            projector: Complex64::new(1.0, 0.0),
        }];

        let trace = getgtr(&gg, 1, &channels).expect("trace should compute");
        assert_close(trace.value, Complex64::new(1.0, 0.0), 1.0e-12);
    }

    #[test]
    fn radial_amplitude_scales_trace_quadratically() {
        let gg = identity_matrix(1);

        let base = getgtr(
            &gg,
            1,
            &[ChannelDescriptor {
                l: 0,
                radial: Complex64::new(1.0, 0.0),
                projector: Complex64::new(1.0, 0.0),
            }],
        )
        .expect("base trace should compute")
        .value;

        let scaled = getgtr(
            &gg,
            1,
            &[ChannelDescriptor {
                l: 0,
                radial: Complex64::new(2.0, 0.0),
                projector: Complex64::new(1.0, 0.0),
            }],
        )
        .expect("scaled trace should compute")
        .value;

        assert_close(scaled, base * 4.0, 1.0e-12);
    }

    #[test]
    fn repeated_calls_are_deterministic() {
        let gg = vec![
            vec![Complex64::new(1.0, 0.0), Complex64::new(0.1, 0.2)],
            vec![Complex64::new(0.1, -0.2), Complex64::new(0.8, 0.0)],
        ];
        let channels = vec![ChannelDescriptor {
            l: 0,
            radial: Complex64::new(1.3, -0.1),
            projector: Complex64::new(0.9, 0.2),
        }];

        let first = getgtr(&gg, 1, &channels).expect("first trace should compute");
        let second = getgtr(&gg, 1, &channels).expect("second trace should compute");

        assert_close(first.value, second.value, 1.0e-14);
    }
}
