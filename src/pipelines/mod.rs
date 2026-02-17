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
    matches!(
        module,
        PipelineModule::Rdinp
            | PipelineModule::Pot
            | PipelineModule::Screen
            | PipelineModule::SelfEnergy
            | PipelineModule::Crpa
            | PipelineModule::Xsph
            | PipelineModule::Path
            | PipelineModule::Fms
            | PipelineModule::Band
            | PipelineModule::Ldos
            | PipelineModule::Compton
            | PipelineModule::Debye
            | PipelineModule::Dmdw
    )
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
        PipelineModule::Screen => RuntimeScreenExecutor.execute_runtime(request),
        PipelineModule::SelfEnergy => RuntimeSelfExecutor.execute_runtime(request),
        PipelineModule::Crpa => RuntimeCrpaExecutor.execute_runtime(request),
        PipelineModule::Xsph => RuntimeXsphExecutor.execute_runtime(request),
        PipelineModule::Path => RuntimePathExecutor.execute_runtime(request),
        PipelineModule::Fms => RuntimeFmsExecutor.execute_runtime(request),
        PipelineModule::Band => RuntimeBandExecutor.execute_runtime(request),
        PipelineModule::Ldos => RuntimeLdosExecutor.execute_runtime(request),
        PipelineModule::Compton => RuntimeComptonExecutor.execute_runtime(request),
        PipelineModule::Debye => RuntimeDebyeExecutor.execute_runtime(request),
        PipelineModule::Dmdw => RuntimeDmdwExecutor.execute_runtime(request),
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeScreenExecutor;

impl RuntimePipelineExecutor for RuntimeScreenExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        screen::ScreenPipelineScaffold.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeSelfExecutor;

impl RuntimePipelineExecutor for RuntimeSelfExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        self_energy::SelfEnergyPipelineScaffold.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeCrpaExecutor;

impl RuntimePipelineExecutor for RuntimeCrpaExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        crpa::CrpaPipelineScaffold.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeXsphExecutor;

impl RuntimePipelineExecutor for RuntimeXsphExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        xsph::XsphPipelineScaffold.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimePathExecutor;

impl RuntimePipelineExecutor for RuntimePathExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        path::PathPipelineScaffold.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeFmsExecutor;

impl RuntimePipelineExecutor for RuntimeFmsExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        fms::FmsPipelineScaffold.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeBandExecutor;

impl RuntimePipelineExecutor for RuntimeBandExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        band::BandPipelineScaffold.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeLdosExecutor;

impl RuntimePipelineExecutor for RuntimeLdosExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        ldos::LdosPipelineScaffold.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeComptonExecutor;

impl RuntimePipelineExecutor for RuntimeComptonExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        compton::ComptonPipelineScaffold.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeDebyeExecutor;

impl RuntimePipelineExecutor for RuntimeDebyeExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        debye::DebyePipelineScaffold.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeDmdwExecutor;

impl RuntimePipelineExecutor for RuntimeDmdwExecutor {
    fn execute_runtime(&self, request: &PipelineRequest) -> PipelineResult<Vec<PipelineArtifact>> {
        dmdw::DmdwPipelineScaffold.execute(request)
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
        assert!(runtime_compute_engine_available(PipelineModule::Screen));
        assert!(runtime_compute_engine_available(PipelineModule::SelfEnergy));
        assert!(runtime_compute_engine_available(PipelineModule::Crpa));
        assert!(runtime_compute_engine_available(PipelineModule::Xsph));
        assert!(runtime_compute_engine_available(PipelineModule::Path));
        assert!(runtime_compute_engine_available(PipelineModule::Fms));
        assert!(runtime_compute_engine_available(PipelineModule::Band));
        assert!(runtime_compute_engine_available(PipelineModule::Ldos));
        assert!(runtime_compute_engine_available(PipelineModule::Compton));
        assert!(runtime_compute_engine_available(PipelineModule::Debye));
        assert!(runtime_compute_engine_available(PipelineModule::Dmdw));
    }

    #[test]
    fn runtime_dispatch_rejects_modules_without_compute_engines() {
        let request = PipelineRequest::new("FX-RIXS-001", PipelineModule::Rixs, "rixs.inp", "out");
        let error = execute_runtime_pipeline(PipelineModule::Rixs, &request)
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
    fn runtime_dispatch_executes_screen_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(input_dir.join("pot.inp"), POT_INPUT_FIXTURE)
            .expect("pot input should be written");
        std::fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");
        std::fs::write(input_dir.join("ldos.inp"), LDOS_INPUT_FIXTURE)
            .expect("ldos input should be written");
        std::fs::write(input_dir.join("screen.inp"), SCREEN_OVERRIDE_INPUT_FIXTURE)
            .expect("screen override should be written");

        let request = PipelineRequest::new(
            "FX-SCREEN-001",
            PipelineModule::Screen,
            input_dir.join("pot.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Screen, &request)
            .expect("SCREEN runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("wscrn.dat")),
            "SCREEN runtime should emit wscrn.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("logscreen.dat")),
            "SCREEN runtime should emit logscreen.dat"
        );
    }

    #[test]
    fn runtime_dispatch_executes_self_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(input_dir.join("sfconv.inp"), SFCONV_INPUT_FIXTURE)
            .expect("sfconv input should be written");
        std::fs::write(input_dir.join("xmu.dat"), SELF_SPECTRUM_INPUT_FIXTURE)
            .expect("spectrum input should be written");

        let request = PipelineRequest::new(
            "FX-SELF-001",
            PipelineModule::SelfEnergy,
            input_dir.join("sfconv.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::SelfEnergy, &request)
            .expect("SELF runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("selfenergy.dat")),
            "SELF runtime should emit selfenergy.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("sigma.dat")),
            "SELF runtime should emit sigma.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("specfunct.dat")),
            "SELF runtime should emit specfunct.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("logsfconv.dat")),
            "SELF runtime should emit logsfconv.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("xmu.dat")),
            "SELF runtime should emit rewritten spectrum outputs"
        );
    }

    #[test]
    fn runtime_dispatch_executes_crpa_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(input_dir.join("crpa.inp"), CRPA_INPUT_FIXTURE)
            .expect("crpa input should be written");
        std::fs::write(input_dir.join("pot.inp"), POT_INPUT_FIXTURE)
            .expect("pot input should be written");
        std::fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");

        let request = PipelineRequest::new(
            "FX-CRPA-001",
            PipelineModule::Crpa,
            input_dir.join("crpa.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Crpa, &request)
            .expect("CRPA runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("wscrn.dat")),
            "CRPA runtime should emit wscrn.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("logscrn.dat")),
            "CRPA runtime should emit logscrn.dat"
        );
    }

    #[test]
    fn runtime_dispatch_executes_xsph_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("outputs");
        let rdinp_request = PipelineRequest::new(
            "FX-WORKFLOW-XAS-001",
            PipelineModule::Rdinp,
            "feff10/examples/XANES/Cu/feff.inp",
            &output_dir,
        );
        execute_runtime_pipeline(PipelineModule::Rdinp, &rdinp_request)
            .expect("RDINP runtime execution should succeed");

        let pot_request = PipelineRequest::new(
            "FX-WORKFLOW-XAS-001",
            PipelineModule::Pot,
            output_dir.join("pot.inp"),
            &output_dir,
        );
        execute_runtime_pipeline(PipelineModule::Pot, &pot_request)
            .expect("POT runtime execution should succeed");

        let xsph_request = PipelineRequest::new(
            "FX-WORKFLOW-XAS-001",
            PipelineModule::Xsph,
            output_dir.join("xsph.inp"),
            &output_dir,
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Xsph, &xsph_request)
            .expect("XSPH runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("phase.bin")),
            "XSPH runtime should emit phase.bin"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("xsect.dat")),
            "XSPH runtime should emit xsect.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("log2.dat")),
            "XSPH runtime should emit log2.dat"
        );
    }

    #[test]
    fn runtime_dispatch_executes_path_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("outputs");
        let rdinp_request = PipelineRequest::new(
            "FX-WORKFLOW-XAS-001",
            PipelineModule::Rdinp,
            "feff10/examples/XANES/Cu/feff.inp",
            &output_dir,
        );
        execute_runtime_pipeline(PipelineModule::Rdinp, &rdinp_request)
            .expect("RDINP runtime execution should succeed");

        let pot_request = PipelineRequest::new(
            "FX-WORKFLOW-XAS-001",
            PipelineModule::Pot,
            output_dir.join("pot.inp"),
            &output_dir,
        );
        execute_runtime_pipeline(PipelineModule::Pot, &pot_request)
            .expect("POT runtime execution should succeed");

        let xsph_request = PipelineRequest::new(
            "FX-WORKFLOW-XAS-001",
            PipelineModule::Xsph,
            output_dir.join("xsph.inp"),
            &output_dir,
        );
        execute_runtime_pipeline(PipelineModule::Xsph, &xsph_request)
            .expect("XSPH runtime execution should succeed");

        let path_request = PipelineRequest::new(
            "FX-WORKFLOW-XAS-001",
            PipelineModule::Path,
            output_dir.join("paths.inp"),
            &output_dir,
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Path, &path_request)
            .expect("PATH runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("paths.dat")),
            "PATH runtime should emit paths.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("paths.bin")),
            "PATH runtime should emit paths.bin"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("crit.dat")),
            "PATH runtime should emit crit.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("log4.dat")),
            "PATH runtime should emit log4.dat"
        );
    }

    #[test]
    fn runtime_dispatch_executes_fms_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(input_dir.join("fms.inp"), FMS_INPUT_FIXTURE)
            .expect("fms input should be written");
        std::fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");
        std::fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");
        std::fs::write(input_dir.join("phase.bin"), phase_fixture_bytes())
            .expect("phase input should be written");

        let request = PipelineRequest::new(
            "FX-FMS-001",
            PipelineModule::Fms,
            input_dir.join("fms.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Fms, &request)
            .expect("FMS runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("gg.bin")),
            "FMS runtime should emit gg.bin"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("log3.dat")),
            "FMS runtime should emit log3.dat"
        );
    }

    #[test]
    fn runtime_dispatch_executes_band_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(input_dir.join("band.inp"), BAND_INPUT_FIXTURE)
            .expect("band input should be written");
        std::fs::write(input_dir.join("geom.dat"), GEOM_INPUT_FIXTURE)
            .expect("geom input should be written");
        std::fs::write(input_dir.join("global.inp"), GLOBAL_INPUT_FIXTURE)
            .expect("global input should be written");
        std::fs::write(input_dir.join("phase.bin"), phase_fixture_bytes())
            .expect("phase input should be written");

        let request = PipelineRequest::new(
            "FX-BAND-001",
            PipelineModule::Band,
            input_dir.join("band.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Band, &request)
            .expect("BAND runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("bandstructure.dat")),
            "BAND runtime should emit bandstructure.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("logband.dat")),
            "BAND runtime should emit logband.dat"
        );
    }

    #[test]
    fn runtime_dispatch_executes_ldos_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let output_dir = temp.path().join("outputs");
        let rdinp_request = PipelineRequest::new(
            "FX-WORKFLOW-XAS-001",
            PipelineModule::Rdinp,
            "feff10/examples/XANES/Cu/feff.inp",
            &output_dir,
        );
        execute_runtime_pipeline(PipelineModule::Rdinp, &rdinp_request)
            .expect("RDINP runtime execution should succeed");

        let pot_request = PipelineRequest::new(
            "FX-WORKFLOW-XAS-001",
            PipelineModule::Pot,
            output_dir.join("pot.inp"),
            &output_dir,
        );
        execute_runtime_pipeline(PipelineModule::Pot, &pot_request)
            .expect("POT runtime execution should succeed");

        std::fs::write(output_dir.join("reciprocal.inp"), RECIPROCAL_INPUT_FIXTURE)
            .expect("reciprocal input should be written");

        let ldos_request = PipelineRequest::new(
            "FX-WORKFLOW-XAS-001",
            PipelineModule::Ldos,
            output_dir.join("ldos.inp"),
            &output_dir,
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Ldos, &ldos_request)
            .expect("LDOS runtime execution should succeed");
        assert!(
            artifacts.iter().any(|artifact| {
                artifact
                    .relative_path
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .starts_with("ldos")
            }),
            "LDOS runtime should emit ldosNN.dat outputs"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("logdos.dat")),
            "LDOS runtime should emit logdos.dat"
        );
    }

    #[test]
    fn runtime_dispatch_executes_compton_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(input_dir.join("compton.inp"), COMPTON_INPUT_FIXTURE)
            .expect("compton input should be written");
        std::fs::write(
            input_dir.join("pot.bin"),
            [0_u8, 1_u8, 2_u8, 3_u8, 4_u8, 5_u8],
        )
        .expect("pot input should be written");
        std::fs::write(
            input_dir.join("gg_slice.bin"),
            [6_u8, 7_u8, 8_u8, 9_u8, 10_u8, 11_u8],
        )
        .expect("gg_slice input should be written");

        let request = PipelineRequest::new(
            "FX-COMPTON-001",
            PipelineModule::Compton,
            input_dir.join("compton.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Compton, &request)
            .expect("COMPTON runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("compton.dat")),
            "COMPTON runtime should emit compton.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("jzzp.dat")),
            "COMPTON runtime should emit jzzp.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("rhozzp.dat")),
            "COMPTON runtime should emit rhozzp.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("logcompton.dat")),
            "COMPTON runtime should emit logcompton.dat"
        );
    }

    #[test]
    fn runtime_dispatch_executes_debye_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(input_dir.join("ff2x.inp"), FF2X_INPUT_FIXTURE)
            .expect("ff2x input should be written");
        std::fs::write(input_dir.join("paths.dat"), PATHS_INPUT_FIXTURE)
            .expect("paths input should be written");
        std::fs::write(input_dir.join("feff.inp"), FEFF_INPUT_FIXTURE)
            .expect("feff input should be written");
        std::fs::write(input_dir.join("spring.inp"), SPRING_INPUT_FIXTURE)
            .expect("spring input should be written");

        let request = PipelineRequest::new(
            "FX-DEBYE-001",
            PipelineModule::Debye,
            input_dir.join("ff2x.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Debye, &request)
            .expect("DEBYE runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("s2_em.dat")),
            "DEBYE runtime should emit s2_em.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("s2_rm1.dat")),
            "DEBYE runtime should emit s2_rm1.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("s2_rm2.dat")),
            "DEBYE runtime should emit s2_rm2.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("xmu.dat")),
            "DEBYE runtime should emit xmu.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("chi.dat")),
            "DEBYE runtime should emit chi.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("log6.dat")),
            "DEBYE runtime should emit log6.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("spring.dat")),
            "DEBYE runtime should emit spring.dat"
        );
    }

    #[test]
    fn runtime_dispatch_executes_dmdw_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(input_dir.join("dmdw.inp"), DMDW_INPUT_FIXTURE)
            .expect("dmdw input should be written");
        std::fs::write(
            input_dir.join("feff.dym"),
            [0_u8, 1_u8, 2_u8, 3_u8, 4_u8, 5_u8],
        )
        .expect("feff.dym input should be written");

        let request = PipelineRequest::new(
            "FX-DMDW-001",
            PipelineModule::Dmdw,
            input_dir.join("dmdw.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_pipeline(PipelineModule::Dmdw, &request)
            .expect("DMDW runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("dmdw.out")),
            "DMDW runtime should emit dmdw.out"
        );
    }

    #[test]
    fn runtime_engine_unavailable_error_uses_computation_category() {
        let error = runtime_engine_unavailable_error(PipelineModule::Rixs);
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

    const LDOS_INPUT_FIXTURE: &str = "mldos, lfms2, ixc, ispin, minv, neldos
   0   0   0   0   0     101
rfms2, emin, emax, eimag, rgrd
      6.00000   1000.00000      0.00000     -1.00000      0.05000
rdirec, toler1, toler2
     12.00000      0.00100      0.00100
 lmaxph(0:nph)
   3   3
";

    const SCREEN_OVERRIDE_INPUT_FIXTURE: &str = "ner          40
nei          20
maxl           4
rfms   4.00000000000000
";

    const SFCONV_INPUT_FIXTURE: &str = "msfconv, ipse, ipsk
   1   0   0
wsigk, cen
      0.00000      0.00000
ispec, ipr6
   1   0
cfname
NULL
";

    const SELF_SPECTRUM_INPUT_FIXTURE: &str = "# omega e k mu mu0 chi
    8979.411  -16.765  -1.406  1.46870E-02  1.79897E-02 -3.30270E-03
    8980.979  -15.197  -1.252  2.93137E-02  3.59321E-02 -6.61845E-03
    8982.398  -13.778  -1.093  3.93900E-02  4.92748E-02 -9.88483E-03
";

    const CRPA_INPUT_FIXTURE: &str = " do_CRPA           1
 rcut   1.49000000000000
 l_crpa           3
";

    const FMS_INPUT_FIXTURE: &str = "mfms, idwopt, minv
   1  -1   0
rfms2, rdirec, toler1, toler2
      4.00000      8.00000      0.00100      0.00100
tk, thetad, sig2g
      0.00000      0.00000      0.00300
 lmaxph(0:nph)
   3   3
 the number of decomposi
   -1
";

    const BAND_INPUT_FIXTURE: &str = "mband : calculate bands if = 1
   1
emin, emax, estep : energy mesh
     -8.00000      6.00000      0.05000
nkp : # points in k-path
 120
ikpath : type of k-path
   2
freeprop :  empty lattice if = T
 F
";

    const RECIPROCAL_INPUT_FIXTURE: &str = "ispace
   1
";

    const COMPTON_INPUT_FIXTURE: &str = "run compton module?
           1
pqmax, npq
   5.000000            1000
ns, nphi, nz, nzp
  32  32  32 120
smax, phimax, zmax, zpmax
      0.00000      6.28319      0.00000     10.00000
jpq? rhozzp? force_recalc_jzzp?
 T T F
window_type (0=Step, 1=Hann), window_cutoff
           1  0.0000000E+00
temperature (in eV)
      0.00000
set_chemical_potential? chemical_potential(eV)
 F  0.0000000E+00
rho_xy? rho_yz? rho_xz? rho_vol? rho_line?
 F F F F F
qhat_x qhat_y qhat_z
  0.000000000000000E+000  0.000000000000000E+000   1.00000000000000
";

    const FF2X_INPUT_FIXTURE: &str = "mchi, ispec, idwopt, ipr6, mbconv, absolu, iGammaCH
   1   0   2   0   0   0   0
vrcorr, vicorr, s02, critcw
      0.00000      0.00000      1.00000      4.00000
tk, thetad, alphat, thetae, sig2g
    450.00000    315.00000      0.00000      0.00000      0.00000
momentum transfer
      0.00000      0.00000      0.00000
 the number of decomposi
   -1
";

    const DMDW_INPUT_FIXTURE: &str =
        "   1\n   6\n   1    450.000\n   0\nfeff.dym\n   1\n   2   1   0          29.78\n";

    const PATHS_INPUT_FIXTURE: &str =
        "PATH  Rmax= 8.000,  Keep_limit= 0.00, Heap_limit 0.00  Pwcrit= 2.50%
 -----------------------------------------------------------------------
     1    2  12.000  index, nleg, degeneracy, r=  2.5323
     2    3  48.000  index, nleg, degeneracy, r=  3.7984
     3    2  24.000  index, nleg, degeneracy, r=  4.3860
";

    const FEFF_INPUT_FIXTURE: &str = "TITLE Cu DEBYE RM Method
EDGE K
EXAFS 15.0
POTENTIALS
    0   29   Cu
    1   29   Cu
ATOMS
    0.00000    0.00000    0.00000    0   Cu  0.00000    0
    1.79059    0.00000    1.79059    1   Cu  2.53228    1
    0.00000    1.79059    1.79059    1   Cu  2.53228    2
END
";

    const SPRING_INPUT_FIXTURE: &str = "*\tres\twmax\tdosfit\tacut
 VDOS\t0.03\t0.5\t1

 STRETCHES
 *\ti\tj\tk_ij\tdR_ij (%)
\t0\t1\t27.9\t2.
";

    const GLOBAL_INPUT_FIXTURE: &str = " nabs, iphabs - CFAVERAGE data
       1       0 100000.00000
 ipol, ispin, le2, elpty, angks, l2lp, do_nrixs, ldecmx, lj
    0    0    0      0.0000      0.0000    0    0   -1   -1
evec xivec spvec
      0.00000      0.00000      1.00000
";

    fn phase_fixture_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(super::xsph::XSPH_PHASE_BINARY_MAGIC);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&6_u32.to_le_bytes());
        bytes.extend_from_slice(&128_u32.to_le_bytes());
        bytes.extend_from_slice(&1_i32.to_le_bytes());
        bytes.extend_from_slice(&0_i32.to_le_bytes());
        bytes.extend_from_slice(&(-25.0_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.15_f64).to_le_bytes());
        bytes.extend_from_slice(&(0.2_f64).to_le_bytes());
        bytes
    }
}
