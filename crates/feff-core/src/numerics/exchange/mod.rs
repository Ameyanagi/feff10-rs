#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeModel {
    HedinLundqvist,
    DiracHara,
    VonBarthHedin,
    PerdewZunger,
}

impl ExchangeModel {
    /// Map FEFF `ixc`/`index` values to local exchange-model families.
    pub fn from_feff_ixc(ixc: i32) -> Self {
        match ixc.rem_euclid(10) {
            0 | 5 => Self::HedinLundqvist,
            1 | 3 | 6 => Self::DiracHara,
            2 | 7 => Self::VonBarthHedin,
            4 | 8 => Self::PerdewZunger,
            _ => Self::HedinLundqvist,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExchangeEvaluationInput {
    pub model: ExchangeModel,
    pub electron_density: f64,
    pub energy: f64,
    pub wave_number: f64,
}

impl ExchangeEvaluationInput {
    pub fn new(model: ExchangeModel, electron_density: f64, energy: f64, wave_number: f64) -> Self {
        Self {
            model,
            electron_density,
            energy,
            wave_number,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExchangeEvaluation {
    pub real: f64,
    pub imaginary: f64,
}

impl ExchangeEvaluation {
    pub const ZERO: Self = Self {
        real: 0.0,
        imaginary: 0.0,
    };
}

pub trait ExchangePotentialApi {
    fn evaluate(&self, input: ExchangeEvaluationInput) -> ExchangeEvaluation;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExchangePotential;

impl ExchangePotentialApi for ExchangePotential {
    fn evaluate(&self, _input: ExchangeEvaluationInput) -> ExchangeEvaluation {
        // Story US-013 wires API surfaces only; model-specific kernels are ported in later stories.
        ExchangeEvaluation::ZERO
    }
}

pub fn evaluate_exchange_potential(input: ExchangeEvaluationInput) -> ExchangeEvaluation {
    ExchangePotential.evaluate(input)
}

#[cfg(test)]
mod tests {
    use super::{
        evaluate_exchange_potential, ExchangeEvaluation, ExchangeEvaluationInput, ExchangeModel,
    };

    #[test]
    fn maps_feff_ixc_to_exchange_models() {
        assert_eq!(
            ExchangeModel::from_feff_ixc(0),
            ExchangeModel::HedinLundqvist
        );
        assert_eq!(
            ExchangeModel::from_feff_ixc(5),
            ExchangeModel::HedinLundqvist
        );
        assert_eq!(
            ExchangeModel::from_feff_ixc(10),
            ExchangeModel::HedinLundqvist
        );

        assert_eq!(ExchangeModel::from_feff_ixc(1), ExchangeModel::DiracHara);
        assert_eq!(ExchangeModel::from_feff_ixc(3), ExchangeModel::DiracHara);
        assert_eq!(ExchangeModel::from_feff_ixc(13), ExchangeModel::DiracHara);

        assert_eq!(
            ExchangeModel::from_feff_ixc(2),
            ExchangeModel::VonBarthHedin
        );
        assert_eq!(ExchangeModel::from_feff_ixc(4), ExchangeModel::PerdewZunger);
    }

    #[test]
    fn default_exchange_evaluator_returns_zero_placeholder() {
        let input = ExchangeEvaluationInput::new(ExchangeModel::PerdewZunger, 0.125, -3.4, 2.8);
        assert_eq!(evaluate_exchange_potential(input), ExchangeEvaluation::ZERO);
    }
}
