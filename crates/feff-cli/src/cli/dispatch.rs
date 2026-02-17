use feff_core::domain::ComputeModule;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub(super) struct ModuleCommandSpec {
    pub(super) command: &'static str,
    pub(super) module: ComputeModule,
    pub(super) input_artifact: &'static str,
}

pub(super) const MODULE_COMMANDS: [ModuleCommandSpec; 16] = [
    ModuleCommandSpec {
        command: "rdinp",
        module: ComputeModule::Rdinp,
        input_artifact: "feff.inp",
    },
    ModuleCommandSpec {
        command: "pot",
        module: ComputeModule::Pot,
        input_artifact: "pot.inp",
    },
    ModuleCommandSpec {
        command: "xsph",
        module: ComputeModule::Xsph,
        input_artifact: "xsph.inp",
    },
    ModuleCommandSpec {
        command: "path",
        module: ComputeModule::Path,
        input_artifact: "paths.inp",
    },
    ModuleCommandSpec {
        command: "fms",
        module: ComputeModule::Fms,
        input_artifact: "fms.inp",
    },
    ModuleCommandSpec {
        command: "band",
        module: ComputeModule::Band,
        input_artifact: "band.inp",
    },
    ModuleCommandSpec {
        command: "ldos",
        module: ComputeModule::Ldos,
        input_artifact: "ldos.inp",
    },
    ModuleCommandSpec {
        command: "rixs",
        module: ComputeModule::Rixs,
        input_artifact: "rixs.inp",
    },
    ModuleCommandSpec {
        command: "crpa",
        module: ComputeModule::Crpa,
        input_artifact: "crpa.inp",
    },
    ModuleCommandSpec {
        command: "compton",
        module: ComputeModule::Compton,
        input_artifact: "compton.inp",
    },
    ModuleCommandSpec {
        command: "ff2x",
        module: ComputeModule::Debye,
        input_artifact: "ff2x.inp",
    },
    ModuleCommandSpec {
        command: "dmdw",
        module: ComputeModule::Dmdw,
        input_artifact: "dmdw.inp",
    },
    ModuleCommandSpec {
        command: "screen",
        module: ComputeModule::Screen,
        input_artifact: "pot.inp",
    },
    ModuleCommandSpec {
        command: "sfconv",
        module: ComputeModule::SelfEnergy,
        input_artifact: "sfconv.inp",
    },
    ModuleCommandSpec {
        command: "eels",
        module: ComputeModule::Eels,
        input_artifact: "eels.inp",
    },
    ModuleCommandSpec {
        command: "fullspectrum",
        module: ComputeModule::FullSpectrum,
        input_artifact: "fullspectrum.inp",
    },
];

pub(super) const SERIAL_CHAIN_ORDER: [ComputeModule; 16] = [
    ComputeModule::Rdinp,
    ComputeModule::Pot,
    ComputeModule::Screen,
    ComputeModule::SelfEnergy,
    ComputeModule::Eels,
    ComputeModule::Xsph,
    ComputeModule::Band,
    ComputeModule::Ldos,
    ComputeModule::Rixs,
    ComputeModule::Crpa,
    ComputeModule::Path,
    ComputeModule::Debye,
    ComputeModule::Dmdw,
    ComputeModule::Fms,
    ComputeModule::Compton,
    ComputeModule::FullSpectrum,
];

pub(super) fn module_command_spec(command: &str) -> Option<ModuleCommandSpec> {
    MODULE_COMMANDS
        .iter()
        .copied()
        .find(|spec| spec.command == command)
}

pub(super) fn module_command_for_module(module: ComputeModule) -> Option<ModuleCommandSpec> {
    MODULE_COMMANDS
        .iter()
        .copied()
        .find(|spec| spec.module == module)
}

pub(super) fn command_alias_from_program_name(program_name: &str) -> Option<&'static str> {
    let executable_name = Path::new(program_name)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program_name);
    let normalized = executable_name
        .strip_suffix(".exe")
        .unwrap_or(executable_name);

    if normalized == "feff10-rs" {
        return None;
    }

    if normalized == "feff" || normalized == "feffmpi" {
        return Some(if normalized == "feff" {
            "feff"
        } else {
            "feffmpi"
        });
    }

    module_command_spec(normalized).map(|spec| spec.command)
}

pub(super) fn parse_compute_module(token: &str) -> Option<ComputeModule> {
    match token.to_ascii_uppercase().as_str() {
        "RDINP" => Some(ComputeModule::Rdinp),
        "POT" => Some(ComputeModule::Pot),
        "PATH" => Some(ComputeModule::Path),
        "FMS" => Some(ComputeModule::Fms),
        "XSPH" => Some(ComputeModule::Xsph),
        "BAND" => Some(ComputeModule::Band),
        "LDOS" => Some(ComputeModule::Ldos),
        "RIXS" => Some(ComputeModule::Rixs),
        "CRPA" => Some(ComputeModule::Crpa),
        "COMPTON" => Some(ComputeModule::Compton),
        "DEBYE" => Some(ComputeModule::Debye),
        "DMDW" => Some(ComputeModule::Dmdw),
        "SCREEN" => Some(ComputeModule::Screen),
        "SELF" => Some(ComputeModule::SelfEnergy),
        "EELS" => Some(ComputeModule::Eels),
        "FULLSPECTRUM" => Some(ComputeModule::FullSpectrum),
        _ => None,
    }
}
