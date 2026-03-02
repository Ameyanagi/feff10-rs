use std::str::FromStr;

/// Individual FEFF calculation stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    Rdinp,
    Dmdw,
    Atomic,
    Pot,
    Ldos,
    Screen,
    Crpa,
    Opconsat,
    Xsph,
    Fms,
    Mkgtr,
    Path,
    Genfmt,
    Ff2x,
    Sfconv,
    Compton,
    Eels,
    Rhorrp,
}

impl Stage {
    pub fn all() -> &'static [Stage] {
        &[
            Stage::Rdinp,
            Stage::Dmdw,
            Stage::Atomic,
            Stage::Pot,
            Stage::Ldos,
            Stage::Screen,
            Stage::Crpa,
            Stage::Opconsat,
            Stage::Xsph,
            Stage::Fms,
            Stage::Mkgtr,
            Stage::Path,
            Stage::Genfmt,
            Stage::Ff2x,
            Stage::Sfconv,
            Stage::Compton,
            Stage::Eels,
            Stage::Rhorrp,
        ]
    }

    /// Name of the executable for this stage.
    pub fn executable_name(&self) -> &'static str {
        match self {
            Stage::Rdinp => "rdinp",
            Stage::Dmdw => "dmdw",
            Stage::Atomic => "atomic",
            Stage::Pot => "pot",
            Stage::Ldos => "ldos",
            Stage::Screen => "screen",
            Stage::Crpa => "crpa",
            Stage::Opconsat => "opconsat",
            Stage::Xsph => "xsph",
            Stage::Fms => "fms",
            Stage::Mkgtr => "mkgtr",
            Stage::Path => "path",
            Stage::Genfmt => "genfmt",
            Stage::Ff2x => "ff2x",
            Stage::Sfconv => "sfconv",
            Stage::Compton => "compton",
            Stage::Eels => "eels",
            Stage::Rhorrp => "rhorrp",
        }
    }

    /// The canonical pipeline order.
    pub fn default_pipeline() -> Vec<Stage> {
        Self::all().to_vec()
    }

    /// Which CONTROL flag index (0-5) controls this stage.
    /// Stages in the same group share a CONTROL flag.
    pub fn control_index(&self) -> usize {
        match self {
            Stage::Rdinp => 0,
            Stage::Dmdw
            | Stage::Atomic
            | Stage::Pot
            | Stage::Ldos
            | Stage::Screen
            | Stage::Crpa
            | Stage::Opconsat => 1,
            Stage::Xsph => 2,
            Stage::Fms | Stage::Mkgtr | Stage::Path => 3,
            Stage::Genfmt => 4,
            Stage::Ff2x | Stage::Sfconv | Stage::Compton | Stage::Eels | Stage::Rhorrp => 5,
        }
    }
}

impl Stage {
    /// Call the corresponding FEFF10 Fortran subroutine via FFI.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// - The current working directory is set to the FEFF working directory
    /// - No other FEFF stage is running concurrently (Fortran global state)
    pub unsafe fn call_ffi(&self) {
        // SAFETY: Each call invokes a Fortran subroutine that operates on files
        // in the current working directory. The caller guarantees cwd is set
        // correctly and no concurrent FEFF calls are in progress.
        unsafe {
            match self {
                Stage::Rdinp => feff10_sys::feff_rdinp(),
                Stage::Dmdw => feff10_sys::feff_dmdw(),
                Stage::Atomic => feff10_sys::feff_atomic(),
                Stage::Pot => feff10_sys::feff_pot(),
                Stage::Ldos => feff10_sys::feff_ldos(),
                Stage::Screen => feff10_sys::feff_screen(),
                Stage::Crpa => feff10_sys::feff_crpa(),
                Stage::Opconsat => feff10_sys::feff_opconsat(),
                Stage::Xsph => feff10_sys::feff_xsph(),
                Stage::Fms => feff10_sys::feff_fms(),
                Stage::Mkgtr => feff10_sys::feff_mkgtr(),
                Stage::Path => feff10_sys::feff_path(),
                Stage::Genfmt => feff10_sys::feff_genfmt(),
                Stage::Ff2x => feff10_sys::feff_ff2x(),
                Stage::Sfconv => feff10_sys::feff_sfconv(),
                Stage::Compton => feff10_sys::feff_compton(),
                Stage::Eels => feff10_sys::feff_eels(),
                Stage::Rhorrp => feff10_sys::feff_rhorrp(),
            }
        }
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.executable_name())
    }
}

impl FromStr for Stage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "rdinp" => Ok(Stage::Rdinp),
            "dmdw" => Ok(Stage::Dmdw),
            "atomic" => Ok(Stage::Atomic),
            "pot" => Ok(Stage::Pot),
            "ldos" => Ok(Stage::Ldos),
            "screen" => Ok(Stage::Screen),
            "crpa" => Ok(Stage::Crpa),
            "opconsat" => Ok(Stage::Opconsat),
            "xsph" => Ok(Stage::Xsph),
            "fms" => Ok(Stage::Fms),
            "mkgtr" => Ok(Stage::Mkgtr),
            "path" => Ok(Stage::Path),
            "genfmt" => Ok(Stage::Genfmt),
            "ff2x" => Ok(Stage::Ff2x),
            "sfconv" => Ok(Stage::Sfconv),
            "compton" => Ok(Stage::Compton),
            "eels" => Ok(Stage::Eels),
            "rhorrp" => Ok(Stage::Rhorrp),
            _ => Err(format!("unknown stage '{s}'")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Stage;

    #[test]
    fn parses_stage_case_insensitive() {
        let stage: Stage = "PoT".parse().unwrap();
        assert_eq!(stage, Stage::Pot);
    }

    #[test]
    fn rejects_unknown_stage() {
        let err = "badstage".parse::<Stage>().unwrap_err();
        assert!(err.contains("unknown stage"));
    }

    #[test]
    fn all_stages_count() {
        assert_eq!(Stage::all().len(), 18);
    }

    #[test]
    fn all_stages_unique_names() {
        let names: Vec<_> = Stage::all().iter().map(|s| s.executable_name()).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "duplicate stage names found");
    }

    #[test]
    fn display_matches_executable_name() {
        for stage in Stage::all() {
            assert_eq!(format!("{stage}"), stage.executable_name());
        }
    }

    #[test]
    fn round_trip_parse_display() {
        for stage in Stage::all() {
            let name = stage.executable_name();
            let parsed: Stage = name.parse().unwrap();
            assert_eq!(*stage, parsed);
        }
    }

    #[test]
    fn control_index_in_range() {
        for stage in Stage::all() {
            assert!(
                stage.control_index() <= 5,
                "{} has control_index {} > 5",
                stage,
                stage.control_index()
            );
        }
    }

    #[test]
    fn default_pipeline_matches_all() {
        assert_eq!(Stage::default_pipeline(), Stage::all().to_vec());
    }
}
