use super::traits::{ModuleExecutor, RuntimeModuleExecutor};
use crate::domain::{FeffError, ComputeArtifact, ComputeModule, ComputeRequest, ComputeResult};

pub fn runtime_compute_engine_available(module: ComputeModule) -> bool {
    matches!(
        module,
        ComputeModule::Rdinp
            | ComputeModule::Pot
            | ComputeModule::Screen
            | ComputeModule::SelfEnergy
            | ComputeModule::Eels
            | ComputeModule::FullSpectrum
            | ComputeModule::Crpa
            | ComputeModule::Xsph
            | ComputeModule::Path
            | ComputeModule::Fms
            | ComputeModule::Band
            | ComputeModule::Ldos
            | ComputeModule::Rixs
            | ComputeModule::Compton
            | ComputeModule::Debye
            | ComputeModule::Dmdw
    )
}

pub fn runtime_engine_unavailable_error(module: ComputeModule) -> FeffError {
    FeffError::computation(
        "RUN.RUNTIME_ENGINE_UNAVAILABLE",
        format!(
            "runtime compute engine for module {} is not available yet; use validation parity flows until the module true-compute story lands",
            module
        ),
    )
}

pub fn execute_runtime_module(
    module: ComputeModule,
    request: &ComputeRequest,
) -> ComputeResult<Vec<ComputeArtifact>> {
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
        ComputeModule::Rdinp => RuntimeRdinpExecutor.execute_runtime(request),
        ComputeModule::Pot => RuntimePotExecutor.execute_runtime(request),
        ComputeModule::Screen => RuntimeScreenExecutor.execute_runtime(request),
        ComputeModule::SelfEnergy => RuntimeSelfExecutor.execute_runtime(request),
        ComputeModule::Eels => RuntimeEelsExecutor.execute_runtime(request),
        ComputeModule::FullSpectrum => RuntimeFullSpectrumExecutor.execute_runtime(request),
        ComputeModule::Crpa => RuntimeCrpaExecutor.execute_runtime(request),
        ComputeModule::Xsph => RuntimeXsphExecutor.execute_runtime(request),
        ComputeModule::Path => RuntimePathExecutor.execute_runtime(request),
        ComputeModule::Fms => RuntimeFmsExecutor.execute_runtime(request),
        ComputeModule::Band => RuntimeBandExecutor.execute_runtime(request),
        ComputeModule::Ldos => RuntimeLdosExecutor.execute_runtime(request),
        ComputeModule::Rixs => RuntimeRixsExecutor.execute_runtime(request),
        ComputeModule::Compton => RuntimeComptonExecutor.execute_runtime(request),
        ComputeModule::Debye => RuntimeDebyeExecutor.execute_runtime(request),
        ComputeModule::Dmdw => RuntimeDmdwExecutor.execute_runtime(request),
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeRdinpExecutor;

impl RuntimeModuleExecutor for RuntimeRdinpExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::rdinp::RdinpModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimePotExecutor;

impl RuntimeModuleExecutor for RuntimePotExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::pot::PotModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeScreenExecutor;

impl RuntimeModuleExecutor for RuntimeScreenExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::screen::ScreenModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeSelfExecutor;

impl RuntimeModuleExecutor for RuntimeSelfExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::self_energy::SelfEnergyModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeEelsExecutor;

impl RuntimeModuleExecutor for RuntimeEelsExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::eels::EelsModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeFullSpectrumExecutor;

impl RuntimeModuleExecutor for RuntimeFullSpectrumExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::fullspectrum::FullSpectrumModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeCrpaExecutor;

impl RuntimeModuleExecutor for RuntimeCrpaExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::crpa::CrpaModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeXsphExecutor;

impl RuntimeModuleExecutor for RuntimeXsphExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::xsph::XsphModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimePathExecutor;

impl RuntimeModuleExecutor for RuntimePathExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::path::PathModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeFmsExecutor;

impl RuntimeModuleExecutor for RuntimeFmsExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::fms::FmsModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeBandExecutor;

impl RuntimeModuleExecutor for RuntimeBandExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::band::BandModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeLdosExecutor;

impl RuntimeModuleExecutor for RuntimeLdosExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::ldos::LdosModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeRixsExecutor;

impl RuntimeModuleExecutor for RuntimeRixsExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::rixs::RixsModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeComptonExecutor;

impl RuntimeModuleExecutor for RuntimeComptonExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::compton::ComptonModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeDebyeExecutor;

impl RuntimeModuleExecutor for RuntimeDebyeExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::debye::DebyeModule.execute(request)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct RuntimeDmdwExecutor;

impl RuntimeModuleExecutor for RuntimeDmdwExecutor {
    fn execute_runtime(&self, request: &ComputeRequest) -> ComputeResult<Vec<ComputeArtifact>> {
        super::dmdw::DmdwModule.execute(request)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        execute_runtime_module, runtime_compute_engine_available,
        runtime_engine_unavailable_error,
    };
    use crate::domain::{FeffErrorCategory, ComputeModule, ComputeRequest};
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn runtime_dispatch_reports_available_compute_modules() {
        assert!(runtime_compute_engine_available(ComputeModule::Rdinp));
        assert!(runtime_compute_engine_available(ComputeModule::Pot));
        assert!(runtime_compute_engine_available(ComputeModule::Screen));
        assert!(runtime_compute_engine_available(ComputeModule::SelfEnergy));
        assert!(runtime_compute_engine_available(ComputeModule::Eels));
        assert!(runtime_compute_engine_available(
            ComputeModule::FullSpectrum
        ));
        assert!(runtime_compute_engine_available(ComputeModule::Crpa));
        assert!(runtime_compute_engine_available(ComputeModule::Xsph));
        assert!(runtime_compute_engine_available(ComputeModule::Path));
        assert!(runtime_compute_engine_available(ComputeModule::Fms));
        assert!(runtime_compute_engine_available(ComputeModule::Band));
        assert!(runtime_compute_engine_available(ComputeModule::Ldos));
        assert!(runtime_compute_engine_available(ComputeModule::Rixs));
        assert!(runtime_compute_engine_available(ComputeModule::Compton));
        assert!(runtime_compute_engine_available(ComputeModule::Debye));
        assert!(runtime_compute_engine_available(ComputeModule::Dmdw));
    }

    #[test]
    fn runtime_dispatch_executes_rixs_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(
            input_dir.join("rixs.inp"),
            "m_run\n1\ngam_ch, gam_exp(1), gam_exp(2)\n0.0001350512 0.0001450512 0.0001550512\nEMinI, EMaxI, EMinF, EMaxF\n-12.0 18.0 -4.0 16.0\nxmu\n-367493090.02742821\nReadpoles, SkipCalc, MBConv, ReadSigma\nT F F T\nnEdges\n2\nEdge 1\nL3\nEdge 2\nL2\n",
        )
        .expect("rixs input should be written");
        std::fs::write(
            input_dir.join("phase_1.bin"),
            [3_u8, 5_u8, 8_u8, 13_u8, 21_u8, 34_u8, 55_u8, 89_u8],
        )
        .expect("phase_1 input should be written");
        std::fs::write(
            input_dir.join("phase_2.bin"),
            [2_u8, 7_u8, 1_u8, 8_u8, 2_u8, 8_u8, 1_u8, 8_u8],
        )
        .expect("phase_2 input should be written");
        std::fs::write(
            input_dir.join("wscrn_1.dat"),
            "-6.0 0.11 0.95\n-2.0 0.16 1.05\n0.0 0.18 1.15\n3.5 0.23 1.30\n8.0 0.31 1.45\n",
        )
        .expect("wscrn_1 input should be written");
        std::fs::write(
            input_dir.join("wscrn_2.dat"),
            "-5.0 0.09 0.85\n-1.5 0.14 0.95\n1.0 0.17 1.05\n4.0 0.21 1.22\n9.0 0.28 1.36\n",
        )
        .expect("wscrn_2 input should be written");
        std::fs::write(
            input_dir.join("xsect_2.dat"),
            "0.0 1.2 0.1\n2.0 1.0 0.2\n4.0 0.9 0.3\n6.0 0.8 0.4\n8.0 0.7 0.5\n",
        )
        .expect("xsect_2 input should be written");

        let request = ComputeRequest::new(
            "FX-RIXS-001",
            ComputeModule::Rixs,
            input_dir.join("rixs.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::Rixs, &request)
            .expect("RIXS runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("rixs0.dat")),
            "RIXS runtime should emit rixs0.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("rixs1.dat")),
            "RIXS runtime should emit rixs1.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("rixsET.dat")),
            "RIXS runtime should emit rixsET.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("rixsEE.dat")),
            "RIXS runtime should emit rixsEE.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("rixsET-sat.dat")),
            "RIXS runtime should emit rixsET-sat.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("rixsEE-sat.dat")),
            "RIXS runtime should emit rixsEE-sat.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("logrixs.dat")),
            "RIXS runtime should emit logrixs.dat"
        );
    }

    #[test]
    fn runtime_dispatch_rejects_module_mismatch_requests() {
        let request = ComputeRequest::new("FX-001", ComputeModule::Rdinp, "feff.inp", "out");
        let error = execute_runtime_module(ComputeModule::Pot, &request)
            .expect_err("module mismatch should fail before dispatch");
        assert_eq!(error.placeholder(), "INPUT.RUNTIME_MODULE_MISMATCH");
    }

    #[test]
    fn runtime_dispatch_executes_rdinp_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let request = ComputeRequest::new(
            "FX-RDINP-001",
            ComputeModule::Rdinp,
            "feff10/examples/EXAFS/Cu/feff.inp",
            temp.path(),
        );
        let artifacts = execute_runtime_module(ComputeModule::Rdinp, &request)
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

        let request = ComputeRequest::new(
            "FX-POT-001",
            ComputeModule::Pot,
            input_dir.join("pot.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::Pot, &request)
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

        let request = ComputeRequest::new(
            "FX-SCREEN-001",
            ComputeModule::Screen,
            input_dir.join("pot.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::Screen, &request)
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

        let request = ComputeRequest::new(
            "FX-SELF-001",
            ComputeModule::SelfEnergy,
            input_dir.join("sfconv.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::SelfEnergy, &request)
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
    fn runtime_dispatch_executes_eels_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(input_dir.join("eels.inp"), EELS_INPUT_FIXTURE)
            .expect("eels input should be written");
        std::fs::write(input_dir.join("xmu.dat"), EELS_XMU_INPUT_FIXTURE)
            .expect("xmu input should be written");

        let request = ComputeRequest::new(
            "FX-EELS-001",
            ComputeModule::Eels,
            input_dir.join("eels.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::Eels, &request)
            .expect("EELS runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("eels.dat")),
            "EELS runtime should emit eels.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("logeels.dat")),
            "EELS runtime should emit logeels.dat"
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

        let request = ComputeRequest::new(
            "FX-CRPA-001",
            ComputeModule::Crpa,
            input_dir.join("crpa.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::Crpa, &request)
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
        let rdinp_request = ComputeRequest::new(
            "FX-WORKFLOW-XAS-001",
            ComputeModule::Rdinp,
            "feff10/examples/XANES/Cu/feff.inp",
            &output_dir,
        );
        execute_runtime_module(ComputeModule::Rdinp, &rdinp_request)
            .expect("RDINP runtime execution should succeed");

        let pot_request = ComputeRequest::new(
            "FX-WORKFLOW-XAS-001",
            ComputeModule::Pot,
            output_dir.join("pot.inp"),
            &output_dir,
        );
        execute_runtime_module(ComputeModule::Pot, &pot_request)
            .expect("POT runtime execution should succeed");

        let xsph_request = ComputeRequest::new(
            "FX-WORKFLOW-XAS-001",
            ComputeModule::Xsph,
            output_dir.join("xsph.inp"),
            &output_dir,
        );
        let artifacts = execute_runtime_module(ComputeModule::Xsph, &xsph_request)
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
        let rdinp_request = ComputeRequest::new(
            "FX-WORKFLOW-XAS-001",
            ComputeModule::Rdinp,
            "feff10/examples/XANES/Cu/feff.inp",
            &output_dir,
        );
        execute_runtime_module(ComputeModule::Rdinp, &rdinp_request)
            .expect("RDINP runtime execution should succeed");

        let pot_request = ComputeRequest::new(
            "FX-WORKFLOW-XAS-001",
            ComputeModule::Pot,
            output_dir.join("pot.inp"),
            &output_dir,
        );
        execute_runtime_module(ComputeModule::Pot, &pot_request)
            .expect("POT runtime execution should succeed");

        let xsph_request = ComputeRequest::new(
            "FX-WORKFLOW-XAS-001",
            ComputeModule::Xsph,
            output_dir.join("xsph.inp"),
            &output_dir,
        );
        execute_runtime_module(ComputeModule::Xsph, &xsph_request)
            .expect("XSPH runtime execution should succeed");

        let path_request = ComputeRequest::new(
            "FX-WORKFLOW-XAS-001",
            ComputeModule::Path,
            output_dir.join("paths.inp"),
            &output_dir,
        );
        let artifacts = execute_runtime_module(ComputeModule::Path, &path_request)
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

        let request = ComputeRequest::new(
            "FX-FMS-001",
            ComputeModule::Fms,
            input_dir.join("fms.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::Fms, &request)
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

        let request = ComputeRequest::new(
            "FX-BAND-001",
            ComputeModule::Band,
            input_dir.join("band.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::Band, &request)
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
        let rdinp_request = ComputeRequest::new(
            "FX-WORKFLOW-XAS-001",
            ComputeModule::Rdinp,
            "feff10/examples/XANES/Cu/feff.inp",
            &output_dir,
        );
        execute_runtime_module(ComputeModule::Rdinp, &rdinp_request)
            .expect("RDINP runtime execution should succeed");

        let pot_request = ComputeRequest::new(
            "FX-WORKFLOW-XAS-001",
            ComputeModule::Pot,
            output_dir.join("pot.inp"),
            &output_dir,
        );
        execute_runtime_module(ComputeModule::Pot, &pot_request)
            .expect("POT runtime execution should succeed");

        std::fs::write(output_dir.join("reciprocal.inp"), RECIPROCAL_INPUT_FIXTURE)
            .expect("reciprocal input should be written");

        let ldos_request = ComputeRequest::new(
            "FX-WORKFLOW-XAS-001",
            ComputeModule::Ldos,
            output_dir.join("ldos.inp"),
            &output_dir,
        );
        let artifacts = execute_runtime_module(ComputeModule::Ldos, &ldos_request)
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

        let request = ComputeRequest::new(
            "FX-COMPTON-001",
            ComputeModule::Compton,
            input_dir.join("compton.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::Compton, &request)
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

        let request = ComputeRequest::new(
            "FX-DEBYE-001",
            ComputeModule::Debye,
            input_dir.join("ff2x.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::Debye, &request)
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

        let request = ComputeRequest::new(
            "FX-DMDW-001",
            ComputeModule::Dmdw,
            input_dir.join("dmdw.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::Dmdw, &request)
            .expect("DMDW runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("dmdw.out")),
            "DMDW runtime should emit dmdw.out"
        );
    }

    #[test]
    fn runtime_dispatch_executes_fullspectrum_compute_engine() {
        let temp = TempDir::new().expect("tempdir should be created");
        let input_dir = temp.path().join("inputs");
        std::fs::create_dir_all(&input_dir).expect("input dir should exist");
        std::fs::write(
            input_dir.join("fullspectrum.inp"),
            FULLSPECTRUM_INPUT_FIXTURE,
        )
        .expect("fullspectrum input should be written");
        std::fs::write(input_dir.join("xmu.dat"), FULLSPECTRUM_XMU_INPUT_FIXTURE)
            .expect("xmu input should be written");

        let request = ComputeRequest::new(
            "FX-FULLSPECTRUM-001",
            ComputeModule::FullSpectrum,
            input_dir.join("fullspectrum.inp"),
            temp.path().join("outputs"),
        );
        let artifacts = execute_runtime_module(ComputeModule::FullSpectrum, &request)
            .expect("FULLSPECTRUM runtime execution should succeed");
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("xmu.dat")),
            "FULLSPECTRUM runtime should emit xmu.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("osc_str.dat")),
            "FULLSPECTRUM runtime should emit osc_str.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("eps.dat")),
            "FULLSPECTRUM runtime should emit eps.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("drude.dat")),
            "FULLSPECTRUM runtime should emit drude.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("background.dat")),
            "FULLSPECTRUM runtime should emit background.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("fine_st.dat")),
            "FULLSPECTRUM runtime should emit fine_st.dat"
        );
        assert!(
            artifacts
                .iter()
                .any(|artifact| artifact.relative_path == Path::new("logfullspectrum.dat")),
            "FULLSPECTRUM runtime should emit logfullspectrum.dat"
        );
    }

    #[test]
    fn runtime_engine_unavailable_error_uses_computation_category() {
        let error = runtime_engine_unavailable_error(ComputeModule::Rixs);
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

    const EELS_INPUT_FIXTURE: &str = "calculate ELNES?
   1
average? relativistic? cross-terms? Which input?
   0   1   1   1   4
polarizations to be used ; min step max
   1   1   9
beam energy in eV
 300000.00000
beam direction in arbitrary units
      0.00000      1.00000      0.00000
collection and convergence semiangle in rad
      0.00240      0.00000
qmesh - radial and angular grid size
   5   3
detector positions - two angles in rad
      0.00000      0.00000
calculate magic angle if magic=1
   0
energy for magic angle - eV above threshold
      0.00000
";

    const EELS_XMU_INPUT_FIXTURE: &str = "# omega e k mu mu0 chi
8979.411 -16.773 -1.540 5.56205E-06 6.25832E-06 -6.96262E-07
8980.979 -15.204 -1.400 6.61771E-06 7.52318E-06 -9.05473E-07
8982.398 -13.786 -1.260 7.99662E-06 9.19560E-06 -1.19897E-06
";

    const FULLSPECTRUM_INPUT_FIXTURE: &str = " mFullSpectrum
           1
 broadening drude
     0.45000     1.25000
 oscillator epsilon_shift
     1.10000     0.25000
";

    const FULLSPECTRUM_XMU_INPUT_FIXTURE: &str = "# omega e k mu mu0 chi
8956.1761 -40.0000 -2.9103 9.162321E-02 9.102713E-02 5.960831E-04
8956.6084 -39.5677 -2.8908 7.595159E-02 7.534298E-02 6.086083E-04
8957.0407 -39.1354 -2.8711 6.248403E-02 6.186194E-02 6.220848E-04
8957.4730 -38.7031 -2.8512 5.166095E-02 5.102360E-02 6.373535E-04
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
        bytes.extend_from_slice(crate::modules::xsph::XSPH_PHASE_BINARY_MAGIC);
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
