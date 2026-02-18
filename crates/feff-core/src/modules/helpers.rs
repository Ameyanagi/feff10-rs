use crate::domain::{ComputeModule, ComputeRequest, InputCard, InputDeck};
use crate::numerics::{deterministic_argsort, distance3, stable_weighted_mean};

#[derive(Debug, Clone, PartialEq)]
pub struct DistanceShell {
    pub site_index: usize,
    pub radius: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreModuleHelper {
    module: ComputeModule,
}

impl CoreModuleHelper {
    pub fn new(module: ComputeModule) -> Option<Self> {
        if is_core_module(module) {
            Some(Self { module })
        } else {
            None
        }
    }

    pub fn module(&self) -> ComputeModule {
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

pub fn is_core_module(module: ComputeModule) -> bool {
    matches!(
        module,
        ComputeModule::Rdinp
            | ComputeModule::Pot
            | ComputeModule::Path
            | ComputeModule::Fms
            | ComputeModule::Xsph
    )
}

pub fn cards_for_compute_request<'a>(
    deck: &'a InputDeck,
    request: &ComputeRequest,
) -> Vec<&'a InputCard> {
    deck.cards_for_module(request.module)
}

#[cfg(test)]
mod tests {
    use super::{CoreModuleHelper, cards_for_compute_request};
    use crate::domain::{ComputeModule, ComputeRequest, InputCard, InputCardKind, InputDeck};

    #[test]
    fn module_helpers_consume_typed_input_cards() {
        let request = ComputeRequest::new("FX-001", ComputeModule::Compton, "feff.inp", "out");
        let deck = InputDeck {
            cards: vec![
                InputCard::new("TITLE", InputCardKind::Title, vec!["Cu".to_string()], 1),
                InputCard::new("COMPTON", InputCardKind::Compton, Vec::new(), 2),
                InputCard::new("RIXS", InputCardKind::Rixs, Vec::new(), 3),
            ],
        };

        let cards = cards_for_compute_request(&deck, &request);
        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].kind, InputCardKind::Title);
        assert_eq!(cards[1].kind, InputCardKind::Compton);
    }

    #[test]
    fn core_module_helper_is_restricted_to_core_modules() {
        assert!(CoreModuleHelper::new(ComputeModule::Pot).is_some());
        assert!(CoreModuleHelper::new(ComputeModule::Compton).is_none());
    }

    #[test]
    fn core_module_helper_uses_numerics_for_deterministic_shell_order() {
        let scaffold = CoreModuleHelper::new(ComputeModule::Path).expect("core scaffold");
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
    fn core_module_helper_uses_numerics_for_weighted_channel_average() {
        let scaffold = CoreModuleHelper::new(ComputeModule::Fms).expect("core scaffold");
        let average = scaffold
            .weighted_channel_average(&[2.0, 8.0], &[1.0, 3.0])
            .expect("weighted average");

        assert!((average - 6.5).abs() < 1.0e-12);
        assert_eq!(scaffold.weighted_channel_average(&[1.0], &[0.0]), None);
    }
}
