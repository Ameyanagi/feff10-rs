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
        vec![
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

    /// Which CONTROL flag index (0-5) controls this stage.
    /// Stages in the same group share a CONTROL flag.
    pub fn control_index(&self) -> usize {
        match self {
            Stage::Rdinp => 0,
            Stage::Dmdw | Stage::Atomic | Stage::Pot | Stage::Ldos | Stage::Screen
            | Stage::Crpa | Stage::Opconsat => 1,
            Stage::Xsph => 2,
            Stage::Fms | Stage::Mkgtr | Stage::Path => 3,
            Stage::Genfmt => 4,
            Stage::Ff2x | Stage::Sfconv | Stage::Compton | Stage::Eels | Stage::Rhorrp => 5,
        }
    }
}

impl std::fmt::Display for Stage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.executable_name())
    }
}
