pub mod band;
pub mod comparator;
pub mod compton;
pub mod crpa;
pub mod debye;
pub mod dmdw;
pub mod eels;
pub mod fms;
pub mod fullspectrum;
pub mod ldos;
pub mod path;
pub mod pot;
pub mod rdinp;
pub mod regression;
pub mod rixs;
pub mod screen;
pub mod self_energy;
pub mod serialization;
pub mod xsph;

use crate::domain::{
    FeffError, InputCard, InputDeck, PipelineArtifact, PipelineModule, PipelineRequest,
    PipelineResult,
};
use crate::numerics::{deterministic_argsort, distance3, stable_weighted_mean};

pub trait PipelineExecutor {
    fn execute(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>>;
}

pub trait RuntimePipelineExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>>;
}

pub trait ValidationPipelineExecutor {
    fn execute_validation(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<Vec<PipelineArtifact>>;
}

impl<T> ValidationPipelineExecutor for T
where
    T: PipelineExecutor,
{
    fn execute_validation(
        &self,
        request: &PipelineRequest,
    ) -> PipelineResult<Vec<PipelineArtifact>> {
        self.execute(request)
    }
}

pub fn runtime_compute_engine_available(module: PipelineModule) -> bool {
    matches!(module, PipelineModule::Rdinp | PipelineModule::Pot)
}

pub fn runtime_engine_unavailable_error(module: PipelineModule) -> FeffError {
    FeffError::computation(
        "RUN.RUNTIME_ENGINE_UNAVAILABLE",
        format!(
            "runtime compute engine for module {} is not available yet; use validation parity flows until the module true-compute story lands",
            module
        ),
    )
}

pub fn execute_runtime_pipeline(
    module: PipelineModule,
    request: &PipelineRequest,
) -> PipelineResult<Vec<PipelineArtifact>> {
    if request.module != module {
        return Err(FeffError::input_validation(
            "INPUT.RUNTIME_MODULE_MISMATCH",
            format!(
                "runtime dispatcher received module {} for request module {}",
                module, request.module
            ),
        ));
    }

    match module {
        PipelineModule::Rdinp => RuntimeRdinpExecutor.execute_runtime(request),
        PipelineModule::Pot => RuntimePotExecutor.execute_runtime(request),
        _ => Err(runtime_engine_unavailable_error(module)),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeRdinpExecutor;

impl RuntimePipelineExecutor for RuntimeRdinpExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        rdinp::RdinpPipelineScaffold.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimePotExecutor;

impl RuntimePipelineExecutor for RuntimePotExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        pot::PotPipelineScaffold.execute(request)
    }
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
    use super::{
        CorePipelineScaffold, PipelineExecutor, ValidationPipelineExecutor,
        cards_for_pipeline_request, execute_runtime_pipeline, runtime_compute_engine_available,
        runtime_engine_unavailable_error,
    };
    use crate::domain::{
        FeffError, FeffErrorCategory, InputCard, InputCardKind, InputDeck, PipelineArtifact,
        PipelineModule, PipelineRequest,
    };
    use std::path::Path;
    use tempfile::TempDir;

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
    fn validation_executor_trait_adapts_pipeline_executor_types() {
        let request = PipelineRequest::new("FX-001", PipelineModule::Rdinp, "feff.inp", "out");
        let error = FailingExecutor
            .execute_validation(&request)
            .expect_err("validation adapter should preserve errors");
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

    #[test]
    fn runtime_dispatch_reports_available_compute_modules() {
        assert!(runtime_compute_engine_available(PipelineModule::Rdinp));
        assert!(runtime_compute_engine_available(PipelineModule::Pot));
        assert!(!runtime_compute_engine_available(PipelineModule::Xsph));
    }

    #[test]
    fn runtime_dispatch_rejects_modules_without_compute_engines() {
        let request = PipelineRequest::new("FX-XSPH-001", PipelineModule::Xsph, "xsph.inp", "out");
        let error = execute_runtime_pipeline(PipelineModule::Xsph, &request)
            .expect_err("unsupported runtime module should fail");
        assert_eq!(error.placeholder(), "RUN.RUNTIME_ENGINE_UNAVAILABLE");
    }

    #[test]
    fn runtime_dispatch_rejects_module_mismatch_requests() {
        let request = PipelineRequest::new("FX-001", PipelineModule::Rdinp, "feff.inp", "out");
        let error = execute_runtime_pipeline(PipelineModule::Pot, &request)
            .expect_err("module mismatch should fail before dispatch");
        assert_eq!(error.placeholder(), "INPUT.RUNTIME_MODULE_MISMATCH");
    }

    #[test]
    fn runtime_dispatch_executes_rdinp_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let request = PipelineRequest::new(
            "FX-RDINP-001",
            PipelineModule::Rdinp,
            "feff10/examples/EXAFS/Cu/feff.inp",
            temp.path(),
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Rdinp, &request)
            .expect("RDINP runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("pot.inp")),
            "RDINP runtime should emit downstream deck artifacts"
        );
    }

    #[test]
    fn runtime_dispatch_executes_pot_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(input_dir.join("pot.inp"), POT_INPUT_FIXTURE)
            .expect("pot input should be written");
        std::fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");

        let request = PipelineRequest::new(
            "FX-POT-001",
            PipelineModule::Pot,
            input_dir.join("pot.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Pot, &request)
            .expect("POT runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("pot.bin")),
            "POT runtime should emit binary potential artifacts"
        );
    }

    #[test]
    fn runtime_engine_unavailable_error_uses_computation_category() {
        let error = runtime_engine_unavailable_error(PipelineModule::Path);
        assert_eq!(error.category(), FeffErrorCategory::ComputationError);
        assert_eq!(error.placeholder(), "RUN.RUNTIME_ENGINE_UNAVAILABLE");
    }

    const POT_INPUT_FIXTURE: &str = "mpot, nph, ntitle, ihole, ipr1, iafolp, ixc,ispec
   1   1   1   1   0   0   0   1
nmix, nohole, jumprm, inters, nscmt, icoul, lfms1, iunf
   6   2   0   0  30   0   0   0
Cu crystal
gamach, rgrd, ca1, ecv, totvol, rfms1
      1.72919      0.05000      0.20000    -40.00000      0.00000      4.00000
 iz, lmaxsc, xnatph, xion, folp
   29    2      1.00000      0.00000      1.15000
   29    2    100.00000      0.00000      1.15000
";

    const GEOM_INPUT_FIXTURE: &str = "nat, nph =    4    1
    1    2
 iat     x       y        z       iph
 -----------------------------------------------------------------------
   1      0.00000      0.00000      0.00000   0   1
   2      1.80500      1.80500      0.00000   1   1
   3     -1.80500      1.80500      0.00000   1   1
   4      0.00000      1.80500      1.80500   1   1
";
}
