pub mod comparator;
pub mod regression;

use crate::domain::{InputCard, InputDeck, PipelineArtifact, PipelineRequest, PipelineResult};

pub trait PipelineExecutor {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>>;
}

pub fn cards_for_pipeline_request<'a>(
    deck: &'a InputDeck,
    request: &PipelineRequest,
) -> Vec<&'a InputCard> {
    deck.cards_for_module(request.module)
}

#[cfg(test)]
mod tests {
    use super::{PipelineExecutor, cards_for_pipeline_request};
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
}
