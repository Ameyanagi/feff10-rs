pub mod errors;

pub use errors::{
    CompatibilityExitPlaceholder, ComputeResult, FeffError, FeffErrorCategory, FeffResult,
    ParserResult,
};

use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ExecutionMode {
    #[default]
    Serial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComputeModule {
    Rdinp,
    Pot,
    Path,
    Fms,
    Xsph,
    Band,
    Ldos,
    Rixs,
    Crpa,
    Compton,
    Debye,
    Dmdw,
    Screen,
    SelfEnergy,
    Eels,
    FullSpectrum,
}

impl ComputeModule {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Rdinp => "RDINP",
            Self::Pot => "POT",
            Self::Path => "PATH",
            Self::Fms => "FMS",
            Self::Xsph => "XSPH",
            Self::Band => "BAND",
            Self::Ldos => "LDOS",
            Self::Rixs => "RIXS",
            Self::Crpa => "CRPA",
            Self::Compton => "COMPTON",
            Self::Debye => "DEBYE",
            Self::Dmdw => "DMDW",
            Self::Screen => "SCREEN",
            Self::SelfEnergy => "SELF",
            Self::Eels => "EELS",
            Self::FullSpectrum => "FULLSPECTRUM",
        }
    }
}

impl Display for ComputeModule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputeRequest {
    pub fixture_id: String,
    pub module: ComputeModule,
    pub execution_mode: ExecutionMode,
    pub input_path: PathBuf,
    pub output_dir: PathBuf,
}

impl ComputeRequest {
    pub fn new(
        fixture_id: impl Into<String>,
        module: ComputeModule,
        input_path: impl Into<PathBuf>,
        output_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            fixture_id: fixture_id.into(),
            module,
            execution_mode: ExecutionMode::Serial,
            input_path: input_path.into(),
            output_dir: output_dir.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputeArtifact {
    pub relative_path: PathBuf,
}

impl ComputeArtifact {
    pub fn new(relative_path: impl Into<PathBuf>) -> Self {
        Self {
            relative_path: relative_path.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputDeck {
    pub cards: Vec<InputCard>,
}

impl InputDeck {
    pub fn cards_for_module(&self, module: ComputeModule) -> Vec<&InputCard> {
        self.cards
            .iter()
            .filter(|card| card.kind.applies_to_module(module))
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputCard {
    pub keyword: String,
    pub kind: InputCardKind,
    pub values: Vec<String>,
    pub continuations: Vec<InputCardContinuation>,
    pub source_line: usize,
}

impl InputCard {
    pub fn new(
        keyword: impl Into<String>,
        kind: InputCardKind,
        values: Vec<String>,
        source_line: usize,
    ) -> Self {
        Self {
            keyword: keyword.into(),
            kind,
            values,
            continuations: Vec::new(),
            source_line,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputCardContinuation {
    pub source_line: usize,
    pub values: Vec<String>,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputCardKind {
    Title,
    Edge,
    S02,
    Control,
    Print,
    Ldos,
    Exafs,
    Rpath,
    Potentials,
    Potential,
    Atoms,
    End,
    Debye,
    Exchange,
    Scf,
    Corehole,
    Xanes,
    Fms,
    Cif,
    Target,
    Hubbard,
    Unfreezef,
    Rixs,
    Xes,
    Egrid,
    EGrid,
    KGrid,
    Compton,
    Cgrid,
    Rhozzp,
    Opcons,
    Mpse,
    Sfconv,
    Corrections,
    Exelfs,
    Reciprocal,
    Kmesh,
    Strfac,
    Elnes,
    Magic,
    Lattice,
    Crpa,
    Vdos,
    Stretches,
    Screen,
    Band,
    FullSpectrum,
    Mband,
    Nkp,
    Ikpath,
    Freeprop,
    Ner,
    Nei,
    Maxl,
    Irrh,
    Iend,
    Lfxc,
    Emin,
    Emax,
    Eimax,
    Ermin,
    Rfms,
    Nrptx0,
    Msfconv,
    Wsigk,
    Ispec,
    Cfname,
    Mfullspectrum,
    Unknown(String),
}

impl InputCardKind {
    pub fn from_keyword(keyword: &str) -> Self {
        match keyword {
            "TITLE" => Self::Title,
            "EDGE" => Self::Edge,
            "S02" => Self::S02,
            "CONTROL" => Self::Control,
            "PRINT" => Self::Print,
            "LDOS" => Self::Ldos,
            "EXAFS" => Self::Exafs,
            "RPATH" => Self::Rpath,
            "POTENTIALS" => Self::Potentials,
            "POTENTIAL" => Self::Potential,
            "ATOMS" => Self::Atoms,
            "END" => Self::End,
            "DEBYE" => Self::Debye,
            "EXCHANGE" => Self::Exchange,
            "SCF" => Self::Scf,
            "COREHOLE" => Self::Corehole,
            "XANES" => Self::Xanes,
            "FMS" => Self::Fms,
            "CIF" => Self::Cif,
            "TARGET" => Self::Target,
            "HUBBARD" => Self::Hubbard,
            "UNFREEZEF" => Self::Unfreezef,
            "RIXS" => Self::Rixs,
            "XES" => Self::Xes,
            "EGRID" => Self::Egrid,
            "E_GRID" => Self::EGrid,
            "K_GRID" => Self::KGrid,
            "COMPTON" => Self::Compton,
            "CGRID" => Self::Cgrid,
            "RHOZZP" => Self::Rhozzp,
            "OPCONS" => Self::Opcons,
            "MPSE" => Self::Mpse,
            "SFCONV" => Self::Sfconv,
            "CORRECTIONS" => Self::Corrections,
            "EXELFS" => Self::Exelfs,
            "RECIPROCAL" => Self::Reciprocal,
            "KMESH" => Self::Kmesh,
            "STRFAC" => Self::Strfac,
            "ELNES" => Self::Elnes,
            "MAGIC" => Self::Magic,
            "LATTICE" => Self::Lattice,
            "CRPA" => Self::Crpa,
            "VDOS" => Self::Vdos,
            "STRETCHES" => Self::Stretches,
            "SCREEN" => Self::Screen,
            "BAND" => Self::Band,
            "FULLSPECTRUM" => Self::FullSpectrum,
            "MBAND" => Self::Mband,
            "NKP" => Self::Nkp,
            "IKPATH" => Self::Ikpath,
            "FREEPROP" => Self::Freeprop,
            "NER" => Self::Ner,
            "NEI" => Self::Nei,
            "MAXL" => Self::Maxl,
            "IRRH" => Self::Irrh,
            "IEND" => Self::Iend,
            "LFXC" => Self::Lfxc,
            "EMIN" => Self::Emin,
            "EMAX" => Self::Emax,
            "EIMAX" => Self::Eimax,
            "ERMIN" => Self::Ermin,
            "RFMS" => Self::Rfms,
            "NRPTX0" => Self::Nrptx0,
            "MSFCONV" => Self::Msfconv,
            "WSIGK" => Self::Wsigk,
            "ISPEC" => Self::Ispec,
            "CFNAME" => Self::Cfname,
            "MFULLSPECTRUM" => Self::Mfullspectrum,
            _ => Self::Unknown(keyword.to_owned()),
        }
    }

    pub fn applies_to_module(&self, module: ComputeModule) -> bool {
        match self {
            Self::Compton | Self::Cgrid | Self::Rhozzp => {
                matches!(module, ComputeModule::Compton | ComputeModule::FullSpectrum)
            }
            Self::Crpa => matches!(module, ComputeModule::Crpa | ComputeModule::FullSpectrum),
            Self::Rixs | Self::Xes => {
                matches!(module, ComputeModule::Rixs | ComputeModule::FullSpectrum)
            }
            Self::Elnes | Self::Exelfs => {
                matches!(module, ComputeModule::Eels | ComputeModule::FullSpectrum)
            }
            Self::Vdos | Self::Stretches => {
                matches!(module, ComputeModule::Debye | ComputeModule::Dmdw)
            }
            Self::Opcons => matches!(
                module,
                ComputeModule::Screen | ComputeModule::Xsph | ComputeModule::FullSpectrum
            ),
            Self::Mpse | Self::Sfconv => matches!(
                module,
                ComputeModule::SelfEnergy | ComputeModule::Xsph | ComputeModule::FullSpectrum
            ),
            Self::Screen
            | Self::Ner
            | Self::Nei
            | Self::Maxl
            | Self::Irrh
            | Self::Iend
            | Self::Lfxc
            | Self::Emin
            | Self::Emax
            | Self::Eimax
            | Self::Ermin
            | Self::Rfms
            | Self::Nrptx0 => matches!(
                module,
                ComputeModule::Screen | ComputeModule::Xsph | ComputeModule::FullSpectrum
            ),
            Self::Msfconv | Self::Wsigk | Self::Ispec | Self::Cfname => matches!(
                module,
                ComputeModule::SelfEnergy | ComputeModule::Xsph | ComputeModule::FullSpectrum
            ),
            Self::Band | Self::Mband | Self::Nkp | Self::Ikpath | Self::Freeprop => {
                module == ComputeModule::Band
            }
            Self::FullSpectrum | Self::Mfullspectrum => module == ComputeModule::FullSpectrum,
            _ => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ComputeModule, ComputeRequest, ExecutionMode, InputCard, InputCardKind, InputDeck,
    };

    #[test]
    fn compute_request_defaults_to_serial_mode() {
        let request = ComputeRequest::new("FX-001", ComputeModule::Rdinp, "feff.inp", "out");
        assert_eq!(request.execution_mode, ExecutionMode::Serial);
        assert_eq!(request.module.to_string(), "RDINP");
    }

    #[test]
    fn input_deck_card_selection_is_module_aware() {
        let mut deck = InputDeck::default();
        deck.cards.push(InputCard::new(
            "COMPTON",
            InputCardKind::Compton,
            Vec::new(),
            1,
        ));
        deck.cards
            .push(InputCard::new("RIXS", InputCardKind::Rixs, Vec::new(), 2));
        deck.cards.push(InputCard::new(
            "TITLE",
            InputCardKind::Title,
            vec!["Cu".to_string()],
            3,
        ));

        let compton_cards = deck.cards_for_module(ComputeModule::Compton);
        assert_eq!(compton_cards.len(), 2);
        assert_eq!(compton_cards[0].keyword, "COMPTON");
        assert_eq!(compton_cards[1].keyword, "TITLE");

        let rixs_cards = deck.cards_for_module(ComputeModule::Rixs);
        assert_eq!(rixs_cards.len(), 2);
        assert_eq!(rixs_cards[0].keyword, "RIXS");
        assert_eq!(rixs_cards[1].keyword, "TITLE");
    }
}
