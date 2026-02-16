pub mod band;
pub mod comparator;
pub mod compton;
pub mod crpa;
pub mod fms;
pub mod ldos;
pub mod path;
pub mod pot;
pub mod rdinp;
pub mod regression;
pub mod rixs;
pub mod xsph;

use crate::domain::{
    InputCard, InputDeck, PipelineArtifact, PipelineModule, PipelineRequest, PipelineResult,
};
use crate::numerics::{deterministic_argsort, distance3, stable_weighted_mean};

pub trait PipelineExecutor {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct DistanceShell {
    pub site_index: usize,
    pub radius: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CorePipelineScaffold {
    module: PipelineModule,
}

impl CorePipelineScaffold {
    pub fn new(module: PipelineModule) -> Option<Self> {
        if is_core_pipeline_module(module) {
            Some(Self { module })
        } else {
            None
        }
    }

    pub fn module(&self) -> PipelineModule {
        self.module
    }

    pub fn sorted_neighbor_shells(
        &self,
        origin: [f64; 3],
        neighbors: &[[f64; 3]],
    ) -> Vec<DistanceShell> {
        let radii: Vec<f64> = neighbors
            .iter()
            .map(|neighbor| distance3(origin, *neighbor))
            .collect();
        let order = deterministic_argsort(&radii);
        order
            .into_iter()
            .map(|site_index| DistanceShell {
                site_index,
                radius: radii[site_index],
            })
            .collect()
    }

    pub fn weighted_channel_average(
        &self,
        channel_values: &[f64],
        channel_weights: &[f64],
    ) -> Option<f64> {
        stable_weighted_mean(channel_values, channel_weights)
    }
}

pub fn is_core_pipeline_module(module: PipelineModule) -> bool {
    matches!(
        module,
        PipelineModule::Rdinp
            | PipelineModule::Pot
            | PipelineModule::Path
            | PipelineModule::Fms
            | PipelineModule::Xsph
    )
}

pub fn cards_for_pipeline_request<'a>(
    deck: &'a InputDeck,
    request: &PipelineRequest,
) -> Vec<&'a InputCard> {
    deck.cards_for_module(request.module)
}

#[cfg(test)]
mod tests {
    use super::{CorePipelineScaffold, PipelineExecutor, cards_for_pipeline_request};
    use crate::domain::{
        FeffError, FeffErrorCategory, InputCard, InputCardKind, InputDeck, PipelineArtifact,
        PipelineModule, PipelineRequest,
    };

    struct FailingExecutor;

    impl PipelineExecutor for FailingExecutor {
        fn execute(
            &self,
            _request: &PipelineRequest,
        ) -> crate::domain::PipelineResult<Vec<PipelineArtifact>> {
            Err(FeffError::computation(
                "RUN.PIPELINE",
                "module execution failed",
            ))
        }
    }

    #[test]
    fn pipeline_executor_uses_shared_error_types() {
        let request = PipelineRequest::new("FX-001", PipelineModule::Rdinp, "feff.inp", "out");
        let error = FailingExecutor
            .execute(&request)
            .expect_err("executor should fail");
        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.exit_code(), 4);
        assert_eq!(error.placeholder(), "RUN.PIPELINE");
    }

    #[test]
    fn pipeline_helpers_consume_typed_input_cards() {
        let request = PipelineRequest::new("FX-001", PipelineModule::Compton, "feff.inp", "out");
        let deck = InputDeck {
            cards: vec![
                InputCard::new("TITLE", InputCardKind::Title, vec!["Cu".to_string()], 1),
                InputCard::new("COMPTON", InputCardKind::Compton, Vec::new(), 2),
                InputCard::new("RIXS", InputCardKind::Rixs, Vec::new(), 3),
            ],
        };

        let cards = cards_for_pipeline_request(&deck, &request);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].kind, InputCardKind::Title);
        assert_eq!(cards[1].kind, InputCardKind::Compton);
    }

    #[test]
    fn core_pipeline_scaffold_is_restricted_to_core_modules() {
        assert!(CorePipelineScaffold::new(PipelineModule::Pot).is_some());
        assert!(CorePipelineScaffold::new(PipelineModule::Compton).is_none());
    }

    #[test]
    fn core_pipeline_scaffold_uses_numerics_for_deterministic_shell_order() {
        let scaffold = CorePipelineScaffold::new(PipelineModule::Path).expect("core scaffold");
        let shells = scaffold.sorted_neighbor_shells(
            [0.0, 0.0, 0.0],
            &[[0.0, 0.0, 2.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
        );

        assert_eq!(shells.len(), 3);
        assert_eq!(shells[0].site_index, 1);
        assert_eq!(shells[1].site_index, 2);
        assert_eq!(shells[2].site_index, 0);
        assert!((shells[0].radius - 1.0).abs() < 1.0e-12);
        assert!((shells[1].radius - 1.0).abs() < 1.0e-12);
        assert!((shells[2].radius - 2.0).abs() < 1.0e-12);
    }

    #[test]
    fn core_pipeline_scaffold_uses_numerics_for_weighted_channel_average() {
        let scaffold = CorePipelineScaffold::new(PipelineModule::Fms).expect("core scaffold");
        let average = scaffold
            .weighted_channel_average(&[2.0, 8.0], &[1.0, 3.0])
            .expect("weighted average");

        assert!((average - 6.5).abs() < 1.0e-12);
        assert_eq!(scaffold.weighted_channel_average(&[1.0], &[0.0]), None);
    }
}
