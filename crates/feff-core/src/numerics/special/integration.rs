#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SommInput<'a> {
    pub radial_grid: &'a [f64],
    pub dp: &'a [f64],
    pub dq: &'a [f64],
    pub log_step: f64,
    pub near_zero_exponent: f64,
    pub power: i32,
}

impl<'a> SommInput<'a> {
    pub fn new(
        radial_grid: &'a [f64],
        dp: &'a [f64],
        dq: &'a [f64],
        log_step: f64,
        near_zero_exponent: f64,
        power: i32,
    ) -> Self {
        Self {
            radial_grid,
            dp,
            dq,
            log_step,
            near_zero_exponent,
            power,
        }
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SommError {
    #[error("somm integration requires at least 2 radial points, got {actual}")]
    InsufficientPoints { actual: usize },
    #[error("somm input length mismatch: radial={radial}, dp={dp}, dq={dq}")]
    LengthMismatch { radial: usize, dp: usize, dq: usize },
    #[error("somm radial grid entries must be finite and > 0 at index {index}, got {value}")]
    InvalidRadius { index: usize, value: f64 },
    #[error(
        "somm radial grid must be strictly increasing, index {index} has {current} after {previous}"
    )]
    NonIncreasingRadius {
        index: usize,
        previous: f64,
        current: f64,
    },
    #[error("somm parameter '{field}' must be finite, got {value}")]
    NonFiniteParameter { field: &'static str, value: f64 },
    #[error("somm vector '{field}' must contain finite values, index {index} got {value}")]
    NonFiniteVectorValue {
        field: &'static str,
        index: usize,
        value: f64,
    },
    #[error("somm power overflow for power={power}")]
    PowerOverflow { power: i32 },
    #[error(
        "somm correction term is singular for near_zero_exponent={near_zero_exponent}, power={power}, log_step={log_step}"
    )]
    SingularCorrection {
        near_zero_exponent: f64,
        power: i32,
        log_step: f64,
    },
    #[error("somm integration produced a non-finite result")]
    NonFiniteResult,
}

pub trait RadialIntegrationApi {
    fn integrate_somm(&self, input: SommInput<'_>) -> Result<f64, SommError>;
}

/// FEFF `somm.f90` radial-grid integration:
/// integrate `(dp + dq) * r^m` from `r=0` to `r=r(np)` on an exponential grid.
pub fn integrate_somm(input: SommInput<'_>) -> Result<f64, SommError> {
    validate_input(input)?;

    let point_count = input.radial_grid.len();
    let mm = input
        .power
        .checked_add(1)
        .ok_or(SommError::PowerOverflow { power: input.power })?;
    let d1 = input.near_zero_exponent + f64::from(mm);
    let exp_step_minus_one = input.log_step.exp() - 1.0;

    if !d1.is_finite() || !exp_step_minus_one.is_finite() || d1 == 0.0 || d1 == -1.0 {
        return Err(SommError::SingularCorrection {
            near_zero_exponent: input.near_zero_exponent,
            power: input.power,
            log_step: input.log_step,
        });
    }

    let mut positive = 0.0;
    let mut negative = 0.0;

    for index in 0..point_count {
        let mut weighted_radius = input.radial_grid[index].powi(mm);
        if index != 0 && index + 1 != point_count {
            weighted_radius += weighted_radius;
            if (index + 1) % 2 == 0 {
                weighted_radius += weighted_radius;
            }
        }

        accumulate_split(
            input.dp[index] * weighted_radius,
            &mut positive,
            &mut negative,
        );
        accumulate_split(
            input.dq[index] * weighted_radius,
            &mut positive,
            &mut negative,
        );
    }

    let mut integral = input.log_step * (positive + negative) / 3.0;
    let correction_denominator =
        d1 * (d1 + 1.0) * exp_step_minus_one * ((d1 - 1.0) * input.log_step).exp();
    if correction_denominator == 0.0 || !correction_denominator.is_finite() {
        return Err(SommError::SingularCorrection {
            near_zero_exponent: input.near_zero_exponent,
            power: input.power,
            log_step: input.log_step,
        });
    }

    let endpoint_scale =
        input.radial_grid[0] * input.radial_grid[1].powi(input.power) / correction_denominator;
    let leading_correction =
        input.radial_grid[0].powi(mm) * (1.0 + 1.0 / (exp_step_minus_one * (d1 + 1.0))) / d1;
    integral += leading_correction * (input.dp[0] + input.dq[0])
        - endpoint_scale * (input.dp[1] + input.dq[1]);

    if !integral.is_finite() {
        return Err(SommError::NonFiniteResult);
    }

    Ok(integral)
}

fn validate_input(input: SommInput<'_>) -> Result<(), SommError> {
    let radial_len = input.radial_grid.len();
    if radial_len < 2 {
        return Err(SommError::InsufficientPoints { actual: radial_len });
    }
    if input.dp.len() != radial_len || input.dq.len() != radial_len {
        return Err(SommError::LengthMismatch {
            radial: radial_len,
            dp: input.dp.len(),
            dq: input.dq.len(),
        });
    }

    if !input.log_step.is_finite() {
        return Err(SommError::NonFiniteParameter {
            field: "log_step",
            value: input.log_step,
        });
    }
    if !input.near_zero_exponent.is_finite() {
        return Err(SommError::NonFiniteParameter {
            field: "near_zero_exponent",
            value: input.near_zero_exponent,
        });
    }

    for (index, radius) in input.radial_grid.iter().copied().enumerate() {
        if !radius.is_finite() || radius <= 0.0 {
            return Err(SommError::InvalidRadius {
                index,
                value: radius,
            });
        }
        if index > 0 {
            let previous = input.radial_grid[index - 1];
            if radius <= previous {
                return Err(SommError::NonIncreasingRadius {
                    index,
                    previous,
                    current: radius,
                });
            }
        }
    }

    validate_vector("dp", input.dp)?;
    validate_vector("dq", input.dq)?;

    Ok(())
}

fn validate_vector(field: &'static str, values: &[f64]) -> Result<(), SommError> {
    for (index, value) in values.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(SommError::NonFiniteVectorValue {
                field,
                index,
                value,
            });
        }
    }

    Ok(())
}

fn accumulate_split(value: f64, positive: &mut f64, negative: &mut f64) {
    if value < 0.0 {
        *negative += value;
    } else if value > 0.0 {
        *positive += value;
    }
}

#[cfg(test)]
mod tests {
    use super::{integrate_somm, SommError, SommInput};

    #[test]
    fn somm_matches_analytic_power_law_integral() {
        let log_step = 0.01;
        let power = 2;
        let near_zero_exponent = 1.5;
        let coefficient = 0.8;
        let radial_grid = exponential_grid(1.0e-6, log_step, 401);
        let dp: Vec<f64> = radial_grid
            .iter()
            .copied()
            .map(|radius| coefficient * radius.powf(near_zero_exponent))
            .collect();
        let dq = vec![0.0; radial_grid.len()];

        let actual = integrate_somm(SommInput::new(
            &radial_grid,
            &dp,
            &dq,
            log_step,
            near_zero_exponent,
            power,
        ))
        .expect("integration");
        let expected = analytic_power_law_integral(
            coefficient,
            near_zero_exponent,
            power,
            *radial_grid.last().expect("non-empty grid"),
        );

        assert_scalar_close("power law", expected, actual, 1.0e-24, 5.0e-8);
    }

    #[test]
    fn somm_matches_analytic_integral_with_signed_components() {
        let log_step = 0.012;
        let power = 0;
        let near_zero_exponent = 0.7;
        let radial_grid = exponential_grid(2.0e-6, log_step, 301);
        let dp: Vec<f64> = radial_grid
            .iter()
            .copied()
            .map(|radius| 1.2 * radius.powf(near_zero_exponent))
            .collect();
        let dq: Vec<f64> = radial_grid
            .iter()
            .copied()
            .map(|radius| -0.5 * radius.powf(near_zero_exponent))
            .collect();

        let actual = integrate_somm(SommInput::new(
            &radial_grid,
            &dp,
            &dq,
            log_step,
            near_zero_exponent,
            power,
        ))
        .expect("integration");
        let expected = analytic_power_law_integral(
            0.7,
            near_zero_exponent,
            power,
            *radial_grid.last().expect("non-empty grid"),
        );

        assert_scalar_close("signed components", expected, actual, 1.0e-15, 1.0e-8);
    }

    #[test]
    fn somm_matches_feff_reference_fixture_case_one() {
        let log_step = 0.15;
        let power = 1;
        let near_zero_exponent = 0.75;
        let radial_grid = exponential_grid(1.0e-4, log_step, 9);
        let dp: Vec<f64> = radial_grid
            .iter()
            .copied()
            .map(|radius| {
                1.5 * radius.powf(0.75) - 0.4 * radius.powf(1.75) + 0.05 * radius.powf(2.75)
            })
            .collect();
        let dq: Vec<f64> = radial_grid
            .iter()
            .copied()
            .map(|radius| {
                -0.6 * radius.powf(0.75) + 0.2 * radius.powf(1.75) - 0.03 * radius.powf(2.75)
            })
            .collect();

        let actual = integrate_somm(SommInput::new(
            &radial_grid,
            &dp,
            &dq,
            log_step,
            near_zero_exponent,
            power,
        ))
        .expect("integration");
        let expected = 8.874_094_217_456_761e-11;
        assert_scalar_close("fixture one", expected, actual, 1.0e-22, 1.0e-13);
    }

    #[test]
    fn somm_matches_feff_reference_fixture_case_two() {
        let log_step = 0.08;
        let power = 0;
        let near_zero_exponent = 1.2;
        let radial_grid = exponential_grid(8.0e-5, log_step, 12);
        let dp: Vec<f64> = radial_grid
            .iter()
            .copied()
            .map(|radius| 0.3 * radius.powf(1.2) + 0.05 * radius.powf(2.2))
            .collect();
        let dq: Vec<f64> = radial_grid
            .iter()
            .copied()
            .map(|radius| {
                -0.5 * radius.powf(1.2) + 0.2 * radius.powf(2.2) - 0.03 * radius.powf(3.2)
            })
            .collect();

        let actual = integrate_somm(SommInput::new(
            &radial_grid,
            &dp,
            &dq,
            log_step,
            near_zero_exponent,
            power,
        ))
        .expect("integration");
        let expected = -5.784_206_366_586_632e-10;
        assert_scalar_close("fixture two", expected, actual, 1.0e-22, 1.0e-13);
    }

    #[test]
    fn somm_rejects_length_mismatch() {
        let radial_grid = [1.0e-4, 2.0e-4, 3.0e-4];
        let dp = [1.0, 2.0];
        let dq = [0.5, 0.25, 0.125];

        let error = integrate_somm(SommInput::new(&radial_grid, &dp, &dq, 0.1, 0.5, 1))
            .expect_err("length mismatch should fail");
        assert_eq!(
            error,
            SommError::LengthMismatch {
                radial: 3,
                dp: 2,
                dq: 3,
            }
        );
    }

    #[test]
    fn somm_rejects_singular_correction_parameters() {
        let radial_grid = [1.0e-4, 2.0e-4];
        let dp = [1.0, 1.0];
        let dq = [0.0, 0.0];

        let error = integrate_somm(SommInput::new(&radial_grid, &dp, &dq, 0.0, 0.5, 1))
            .expect_err("log_step=0 should fail");
        assert_eq!(
            error,
            SommError::SingularCorrection {
                near_zero_exponent: 0.5,
                power: 1,
                log_step: 0.0,
            }
        );
    }

    fn analytic_power_law_integral(
        coefficient: f64,
        near_zero_exponent: f64,
        power: i32,
        upper_radius: f64,
    ) -> f64 {
        let exponent = near_zero_exponent + f64::from(power) + 1.0;
        coefficient * upper_radius.powf(exponent) / exponent
    }

    fn exponential_grid(start_radius: f64, log_step: f64, count: usize) -> Vec<f64> {
        (0..count)
            .map(|index| start_radius * (log_step * index as f64).exp())
            .collect()
    }

    fn assert_scalar_close(label: &str, expected: f64, actual: f64, abs_tol: f64, rel_tol: f64) {
        let abs_diff = (actual - expected).abs();
        let rel_diff = abs_diff / expected.abs().max(1.0);
        assert!(
            abs_diff <= abs_tol || rel_diff <= rel_tol,
            "{label} expected={expected:.15e} actual={actual:.15e} abs_diff={abs_diff:.15e} rel_diff={rel_diff:.15e} abs_tol={abs_tol:.15e} rel_tol={rel_tol:.15e}"
        );
    }
}
