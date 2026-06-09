use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::error::{Error, ParseError};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParseMode {
    Strict,
}

#[derive(Debug, Clone)]
struct SourceLine {
    line: String,
    line_num: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CardToken(i16);

impl CardToken {
    pub fn id(self) -> i16 {
        self.0
    }

    pub fn canonical_name(self) -> &'static str {
        match self.0 {
            -1 => "END",
            1 => "ATOMS",
            2 => "HOLE",
            3 => "OVERLAP",
            4 => "CONTROL",
            5 => "EXCHANGE",
            6 => "ION",
            7 => "TITLE",
            8 => "FOLP",
            9 => "RPATH",
            10 => "DEBYE",
            11 => "RMULTIPLIER",
            12 => "SS",
            13 => "PRINT",
            14 => "POTENTIALS",
            15 => "NLEG",
            16 => "CRITERIA",
            17 => "NOGEOM",
            18 => "IORDER",
            19 => "PCRITERIA",
            20 => "SIG2",
            21 => "XANES",
            22 => "CORRECTIONS",
            23 => "AFOLP",
            24 => "EXAFS",
            25 => "POLARIZATION",
            26 => "ELLIPTICITY",
            27 => "RGRID",
            28 => "RPHASES",
            29 => "NSTAR",
            30 => "NOHOLE",
            31 => "SIG3",
            32 => "JUMPRM",
            33 => "MBCONV",
            34 => "SPIN",
            35 => "EDGE",
            36 => "SCF",
            37 => "FMS",
            38 => "LDOS",
            39 => "INTERSTITIAL",
            40 => "CFAVERAGE",
            41 => "S02",
            42 => "XES",
            43 => "DANES",
            44 => "FPRIME",
            45 => "RSIGMA",
            46 => "XNCD",
            47 => "MULTIPOLE",
            48 => "UNFREEZEF",
            49 => "TDLDA",
            50 => "PMBSE",
            51 => "MPSE",
            52 => "SFCONV",
            53 => "SELF",
            54 => "SFSE",
            55 => "RCONV",
            56 => "ELNES",
            57 => "EXELFS",
            58 => "MAGIC",
            59 => "ABSOLUTE",
            60 => "SYMMETRY",
            61 => "REAL",
            62 => "RECIPROCAL",
            63 => "SGROUP",
            64 => "LATTICE",
            65 => "KMESH",
            66 => "STRFAC",
            67 => "BANDSTRUCTURE",
            68 => "COREHOLE",
            71 => "TARGET",
            72 => "EGRID",
            73 => "COORDINATES",
            74 => "EXTPOT",
            75 => "CHBROADENING",
            76 => "CHSHIFT",
            77 => "DIMS",
            78 => "NRIXS",
            79 => "LJMAX",
            80 => "LDEC",
            81 => "SETEDGE",
            82 => "EPS0",
            83 => "OPCONS",
            84 => "NUMDENS",
            85 => "PREPS",
            86 => "EGAP",
            87 => "CHWIDTH",
            88 => "MDFF",
            89 => "RESTART",
            90 => "CONFIG",
            91 => "SCREEN",
            92 => "CIF",
            93 => "EQUIVALENCE",
            94 => "COMPTON",
            95 => "RHOZZP",
            96 => "CGRID",
            97 => "CORVAL",
            98 => "SIGGK",
            99 => "TEMP",
            100 => "DENSITY",
            101 => "RIXS",
            102 => "RLPRINT",
            103 => "ICORE",
            104 => "HUBBARD",
            105 => "CRPA",
            106 => "FULLSPECTRUM",
            107 => "SCXC",
            108 => "HIGHZ",
            109 => "SCFTH",
            110 => "WARNION",
            111 => "SCFRAMP",
            112 => "TOLSCF",
            _ => "UNKNOWN",
        }
    }

    fn from_word(word: &str) -> Option<Self> {
        let mut w = word
            .chars()
            .take(4)
            .collect::<String>()
            .to_ascii_uppercase();
        while w.len() < 4 {
            w.push(' ');
        }
        let id = match w.as_str() {
            "ATOM" => 1,
            "HOLE" => 2,
            "OVER" => 3,
            "CONT" => 4,
            "EXCH" => 5,
            "ION " => 6,
            "TITL" => 7,
            "FOLP" => 8,
            "RPAT" | "RMAX" => 9,
            "DEBY" => 10,
            "RMUL" => 11,
            "SS  " => 12,
            "PRIN" => 13,
            "POTE" => 14,
            "NLEG" => 15,
            "CRIT" => 16,
            "NOGE" => 17,
            "IORD" => 18,
            "PCRI" => 19,
            "SIG2" => 20,
            "XANE" => 21,
            "CORR" => 22,
            "AFOL" => 23,
            "EXAF" => 24,
            "POLA" => 25,
            "ELLI" => 26,
            "RGRI" => 27,
            "RPHA" => 28,
            "NSTA" => 29,
            "NOHO" => 30,
            "SIG3" => 31,
            "JUMP" => 32,
            "MBCO" => 33,
            "SPIN" => 34,
            "EDGE" => 35,
            "SCF " => 36,
            "FMS " => 37,
            "LDOS" => 38,
            "INTE" => 39,
            "CFAV" => 40,
            "S02 " => 41,
            "XES " => 42,
            "DANE" => 43,
            "FPRI" => 44,
            "RSIG" => 45,
            "XNCD" | "XMCD" => 46,
            "MULT" => 47,
            "UNFR" => 48,
            "TDLD" => 49,
            "PMBS" => 50,
            "PLAS" | "MPSE" => 51,
            "SO2C" | "SFCO" => 52,
            "SELF" => 53,
            "SFSE" => 54,
            // NOTE: upstream FEFF10 `itoken.f90` compares a 4-char token buffer
            // to the 5-char literal `RCONV`, so this card is effectively unreachable.
            "ELNE" => 56,
            "EXEL" => 57,
            "MAGI" => 58,
            "ABSO" => 59,
            "SYMM" => 60,
            "REAL" => 61,
            "RECI" => 62,
            "SGRO" => 63,
            "LATT" => 64,
            "KMES" => 65,
            "STRF" => 66,
            "BAND" => 67,
            "CORE" => 68,
            "MARK" | "TARG" => 71,
            "EGRI" => 72,
            "COOR" => 73,
            "EXTP" => 74,
            "CHBR" => 75,
            "CHSH" => 76,
            "DIMS" => 77,
            "NRIX" => 78,
            "LJMA" => 79,
            "LDEC" => 80,
            "SETE" => 81,
            "EPS0" => 82,
            "OPCO" => 83,
            "NUMD" => 84,
            "PREP" => 85,
            "EGAP" => 86,
            "CHWI" => 87,
            "MDFF" => 88,
            "REST" => 89,
            "CONF" => 90,
            "SCRE" => 91,
            "CIF " => 92,
            "EQUI" => 93,
            "COMP" => 94,
            "RHOZ" => 95,
            "CGRI" => 96,
            "CORV" => 97,
            "SIGG" => 98,
            "TEMP" => 99,
            "DENS" => 100,
            "RIXS" => 101,
            "RLPR" => 102,
            "ICOR" => 103,
            "HUBB" => 104,
            "CRPA" => 105,
            "FULL" => 106,
            "SCXC" => 107,
            "HIGH" => 108,
            "SCFT" => 109,
            "WARN" => 110,
            "SCFR" => 111,
            "TOLS" => 112,
            "END " => -1,
            _ => 0,
        };
        if id == 0 { None } else { Some(Self(id)) }
    }
}

#[derive(Debug, Clone)]
pub struct InputCard {
    pub token: CardToken,
    pub keyword: String,
    pub line: String,
    pub continuation: Vec<String>,
    pub line_num: usize,
}

#[derive(Debug, Clone)]
pub struct ResolvedFeffInput {
    pub edge: String,
    pub s02: Option<f64>,
    pub control: [u32; 6],
    pub print_flags: [u32; 6],
    pub title: Vec<String>,
    pub cards_set: HashSet<i16>,
    pub potentials: Vec<Potential>,
    pub atoms: Vec<Atom>,
}

#[derive(Debug, Clone)]
pub struct ResolvedFortranInput {
    pub edge: String,
    pub s02: Option<f64>,
    pub control: [u32; 6],
    pub print_flags: [u32; 6],
    pub title: Vec<String>,
    pub cards_set: HashSet<i16>,
    pub spectroscopy: String,
    pub corehole: String,
    pub potentials: Vec<Potential>,
    pub atoms: Vec<Atom>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExafsCard {
    pub xkmax: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XanesCard {
    pub xkmax: Option<f64>,
    pub xkstep: Option<f64>,
    pub vixan: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScfCard {
    pub rfms1: Option<f64>,
    pub lfms1: Option<i32>,
    pub nscmt: Option<i32>,
    pub ca: Option<f64>,
    pub nmix: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FmsCard {
    pub rfms: Option<f64>,
    pub lfms2: Option<i32>,
    pub minv: Option<i32>,
    pub toler1: Option<f64>,
    pub toler2: Option<f64>,
    pub rdirec: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CoreholeCard {
    pub mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExchangeCard {
    pub ixc: Option<i32>,
    pub vr0: Option<f64>,
    pub vi0: Option<f64>,
    pub ixc0: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DebyeCard {
    pub temp: Option<f64>,
    pub thetad: Option<f64>,
    pub idwopt: Option<i32>,
    pub extras: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpathCard {
    pub rmax: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NlegCard {
    pub nleg: Option<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KmeshCard {
    pub nkx: Option<i32>,
    pub nky: Option<i32>,
    pub nkz: Option<i32>,
    pub ktype: Option<i32>,
    pub usesym: Option<i32>,
}

#[derive(Debug, Clone)]
pub enum TypedCard {
    Title(String),
    Edge {
        label: String,
        s02: Option<f64>,
    },
    S02(f64),
    Control([u32; 6]),
    Print([u32; 6]),
    Exafs(ExafsCard),
    Xanes(XanesCard),
    Scf(ScfCard),
    Fms(FmsCard),
    Corehole(CoreholeCard),
    Exchange(ExchangeCard),
    Debye(DebyeCard),
    Rpath(RpathCard),
    Nleg(NlegCard),
    Kmesh(KmeshCard),
    Potentials(Vec<Potential>),
    Atoms(Vec<Atom>),
    Other {
        token: CardToken,
        keyword: String,
        line: String,
        continuation: Vec<String>,
    },
}

#[derive(Debug, Clone)]
enum SectionMode {
    Normal,
    Atoms {
        card_idx: usize,
    },
    Overlap {
        card_idx: usize,
    },
    Potentials {
        card_idx: usize,
    },
    EelsContinuation {
        card_idx: usize,
        step: u8,
    },
    LatticeVectors {
        card_idx: usize,
        remaining: u8,
    },
    NrixsContinuation {
        card_idx: usize,
        remaining: usize,
        qaverage: bool,
    },
    EgridBlock {
        card_idx: usize,
    },
    DensityBlock {
        card_idx: usize,
    },
    ConfigCardBlock {
        card_idx: usize,
        remaining: usize,
    },
}

/// A potential type definition.
#[derive(Debug, Clone)]
pub struct Potential {
    pub ipot: u32,
    pub z: u32,
    pub tag: String,
    pub l_scmt: Option<u32>,
    pub l_fms: Option<u32>,
    pub stoich: Option<f64>,
}

/// An atom in the cluster.
#[derive(Debug, Clone)]
pub struct Atom {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub ipot: u32,
    pub tag: String,
    pub distance: f64,
}

/// Parsed feff.inp file.
#[derive(Debug, Clone)]
pub struct FeffInput {
    pub cards: Vec<InputCard>,
    pub title: Vec<String>,
    pub edge: Option<String>,
    pub s02: Option<f64>,
    pub control: [u32; 6],
    pub print_flags: [u32; 6],
    pub potentials: Vec<Potential>,
    pub atoms: Vec<Atom>,
    /// Raw card lines that we preserve but don't specifically parse.
    /// This ensures round-trip fidelity for cards like DEBYE, EXAFS, XANES, etc.
    pub other_cards: Vec<String>,
}

impl Default for FeffInput {
    fn default() -> Self {
        Self {
            cards: Vec::new(),
            title: Vec::new(),
            edge: None,
            s02: None,
            control: [1; 6],
            print_flags: [0; 6],
            potentials: Vec::new(),
            atoms: Vec::new(),
            other_cards: Vec::new(),
        }
    }
}

impl FeffInput {
    /// Parse a feff.inp file from a string.
    pub fn parse(content: &str) -> Result<Self, Error> {
        let lines = collect_source_lines_from_content(content, std::env::current_dir().ok())?;
        Self::parse_source_lines(lines, ParseMode::Strict)
    }

    /// Parse a feff.inp file from a string with strict validation.
    pub fn parse_strict(content: &str) -> Result<Self, Error> {
        Self::parse(content)
    }

    fn parse_source_lines(lines: Vec<SourceLine>, mode: ParseMode) -> Result<Self, Error> {
        let mut input = FeffInput::default();
        let mut section_mode = SectionMode::Normal;

        for src in lines {
            let line_num = src.line_num;
            let line = untab(&src.line).trim_start().to_string();
            if is_comment_or_blank(&line) {
                continue;
            }

            let words = split_words(&line);
            if words.is_empty() {
                continue;
            }

            let token = CardToken::from_word(&words[0]);
            let mut reprocess = true;
            while reprocess {
                reprocess = false;
                match section_mode {
                    SectionMode::Normal => {
                        let token = token.ok_or_else(|| {
                            parse_error(line_num, format!("unrecognized keyword '{}'", words[0]))
                        })?;
                        let keyword = token.canonical_name().to_string();
                        let card_idx = input.cards.len();
                        input.cards.push(InputCard {
                            token,
                            keyword: keyword.clone(),
                            line: line.clone(),
                            continuation: Vec::new(),
                            line_num,
                        });

                        if token.id() == -1 {
                            if mode == ParseMode::Strict {
                                input.validate_fortran_rules()?;
                            }
                            return Ok(input);
                        }

                        match token.id() {
                            1 => {
                                section_mode = SectionMode::Atoms { card_idx };
                            }
                            3 => {
                                input.other_cards.push(line.clone());
                                section_mode = SectionMode::Overlap { card_idx };
                            }
                            14 => {
                                section_mode = SectionMode::Potentials { card_idx };
                            }
                            56 | 57 => {
                                input.other_cards.push(line.clone());
                                section_mode = SectionMode::EelsContinuation { card_idx, step: 5 };
                            }
                            64 => {
                                input.other_cards.push(line.clone());
                                section_mode = SectionMode::LatticeVectors {
                                    card_idx,
                                    remaining: 3,
                                };
                            }
                            78 => {
                                let nq = parse_i32_token(words.get(1).map_or("", String::as_str))
                                    .ok_or_else(|| {
                                    parse_error(
                                        line_num,
                                        "NRIXS requires an integer nq as first argument",
                                    )
                                })?;
                                let nq_abs = nq.unsigned_abs() as usize;
                                if nq_abs > 1 {
                                    section_mode = SectionMode::NrixsContinuation {
                                        card_idx,
                                        remaining: nq_abs - 1,
                                        qaverage: nq < 0,
                                    };
                                }
                            }
                            72 if words.len() == 1 => {
                                input.other_cards.push(line.clone());
                                section_mode = SectionMode::EgridBlock { card_idx };
                            }
                            100 => {
                                input.other_cards.push(line.clone());
                                section_mode = SectionMode::DensityBlock { card_idx };
                            }
                            90 if words
                                .get(1)
                                .map(|s| s.eq_ignore_ascii_case("card"))
                                .unwrap_or(false) =>
                            {
                                let nlines =
                                    words.get(2).and_then(|s| parse_i32_token(s)).unwrap_or(0);
                                if nlines > 0 {
                                    section_mode = SectionMode::ConfigCardBlock {
                                        card_idx,
                                        remaining: nlines as usize,
                                    };
                                }
                            }
                            _ => {}
                        }

                        match token.id() {
                            7 => {
                                let title = remainder_after_keyword(&line, &words[0]);
                                input.title.push(title.to_string());
                            }
                            35 => {
                                if words.len() < 2 {
                                    return Err(parse_error(
                                        line_num,
                                        "EDGE requires an edge label",
                                    ));
                                }
                                input.edge = Some(words[1].to_ascii_uppercase());
                            }
                            41 => {
                                if words.len() < 2 {
                                    return Err(parse_error(
                                        line_num,
                                        "S02 requires a numeric value",
                                    ));
                                }
                                let val = parse_f64_token(&words[1]).ok_or_else(|| {
                                    parse_error(
                                        line_num,
                                        format!("invalid S02 value '{}'", words[1]),
                                    )
                                })?;
                                input.s02 = Some(val);
                            }
                            4 => {
                                input.control =
                                    parse_control_print_flags(&words, line_num, "CONTROL")?;
                            }
                            13 => {
                                input.print_flags =
                                    parse_control_print_flags(&words, line_num, "PRINT")?;
                            }
                            // non-structural cards are preserved verbatim
                            2 | 3 | 5 | 6 | 8..=12 | 15..=34 | 36..=40 | 42..=68 | 71..=112
                                if token.id() != 3
                                    && token.id() != 56
                                    && token.id() != 57
                                    && token.id() != 64
                                    && !(token.id() == 72 && words.len() == 1)
                                    && token.id() != 100 =>
                            {
                                input.other_cards.push(line.clone());
                            }
                            _ => {}
                        }

                        validate_card_words_fortran(token.id(), &words, line_num)?;
                    }
                    SectionMode::Atoms { card_idx } => {
                        if token.is_some() {
                            section_mode = SectionMode::Normal;
                            reprocess = true;
                            continue;
                        }
                        input.cards[card_idx].continuation.push(line.clone());
                        let atom = parse_atom_line_strict(&line, line_num)?;
                        input.atoms.push(atom);
                    }
                    SectionMode::Overlap { card_idx } => {
                        if token.is_some() {
                            section_mode = SectionMode::Normal;
                            reprocess = true;
                            continue;
                        }
                        input.cards[card_idx].continuation.push(line.clone());
                        input.other_cards.push(line.clone());
                        parse_overlap_line_strict(&line, line_num)?;
                    }
                    SectionMode::Potentials { card_idx } => {
                        if token.is_some() {
                            section_mode = SectionMode::Normal;
                            reprocess = true;
                            continue;
                        }
                        input.cards[card_idx].continuation.push(line.clone());
                        let pot = parse_potential_line_strict(&line, line_num)?;
                        input.potentials.push(pot);
                    }
                    SectionMode::EelsContinuation { card_idx, step } => {
                        input.cards[card_idx].continuation.push(line.clone());
                        input.other_cards.push(line.clone());
                        let skip_orientation =
                            parse_eels_continuation_line_strict(&line, line_num, step)?;
                        let mut next_step = step.saturating_sub(1);
                        if step == 5 && skip_orientation {
                            next_step = next_step.saturating_sub(1);
                        }
                        if next_step == 0 {
                            section_mode = SectionMode::Normal;
                        } else {
                            section_mode = SectionMode::EelsContinuation {
                                card_idx,
                                step: next_step,
                            };
                        }
                    }
                    SectionMode::LatticeVectors {
                        card_idx,
                        mut remaining,
                    } => {
                        input.cards[card_idx].continuation.push(line.clone());
                        input.other_cards.push(line.clone());
                        parse_lattice_vector_line_strict(&line, line_num)?;
                        remaining = remaining.saturating_sub(1);
                        if remaining == 0 {
                            section_mode = SectionMode::Normal;
                        } else {
                            section_mode = SectionMode::LatticeVectors {
                                card_idx,
                                remaining,
                            };
                        }
                    }
                    SectionMode::NrixsContinuation {
                        card_idx,
                        mut remaining,
                        qaverage,
                    } => {
                        input.cards[card_idx].continuation.push(line.clone());
                        input.other_cards.push(line.clone());
                        parse_nrixs_continuation_line_strict(&line, line_num, qaverage)?;
                        remaining = remaining.saturating_sub(1);
                        if remaining == 0 {
                            section_mode = SectionMode::Normal;
                        } else {
                            section_mode = SectionMode::NrixsContinuation {
                                card_idx,
                                remaining,
                                qaverage,
                            };
                        }
                    }
                    SectionMode::EgridBlock { card_idx } => {
                        if token.is_some() {
                            section_mode = SectionMode::Normal;
                            reprocess = true;
                            continue;
                        }
                        input.cards[card_idx].continuation.push(line.clone());
                        input.other_cards.push(line.clone());
                    }
                    SectionMode::DensityBlock { card_idx } => {
                        if token.is_some() {
                            section_mode = SectionMode::Normal;
                            reprocess = true;
                            continue;
                        }
                        input.cards[card_idx].continuation.push(line.clone());
                        input.other_cards.push(line.clone());
                    }
                    SectionMode::ConfigCardBlock {
                        card_idx,
                        mut remaining,
                    } => {
                        input.cards[card_idx].continuation.push(line.clone());
                        remaining = remaining.saturating_sub(1);
                        if remaining == 0 {
                            section_mode = SectionMode::Normal;
                        } else {
                            section_mode = SectionMode::ConfigCardBlock {
                                card_idx,
                                remaining,
                            };
                        }
                    }
                }
            }
        }

        if mode == ParseMode::Strict {
            input.validate_fortran_rules()?;
        }
        Ok(input)
    }

    /// Parse from a file path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        let lines = collect_source_lines_from_file(path.as_ref())?;
        Self::parse_source_lines(lines, ParseMode::Strict)
    }

    /// Parse from a file path with strict validation.
    pub fn from_file_strict(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::from_file(path)
    }

    fn effective_cards_set(&self) -> HashSet<i16> {
        let mut cards_set = HashSet::new();
        for card in &self.cards {
            cards_set.insert(card.token.id());
        }
        for card in &self.other_cards {
            let words = split_words(card);
            if let Some(first) = words.first()
                && let Some(tok) = CardToken::from_word(first)
            {
                cards_set.insert(tok.id());
            }
        }
        if !self.title.is_empty() {
            cards_set.insert(7);
        }
        if self.edge.is_some() {
            cards_set.insert(35);
        }
        if self.s02.is_some() {
            cards_set.insert(41);
        }
        if !self.potentials.is_empty() {
            cards_set.insert(14);
        }
        if !self.atoms.is_empty() {
            cards_set.insert(1);
        }
        cards_set
    }

    /// Resolve parser defaults to an explicit model.
    ///
    /// This captures the currently implemented Fortran-default parity:
    /// - EDGE defaults to `K`
    /// - CONTROL defaults to `[1; 6]`
    /// - PRINT defaults to `[0; 6]`
    pub fn resolve_defaults(&self) -> ResolvedFeffInput {
        let resolved = self.resolve_fortran_defaults();
        ResolvedFeffInput {
            edge: resolved.edge,
            s02: resolved.s02,
            control: resolved.control,
            print_flags: resolved.print_flags,
            title: resolved.title,
            cards_set: resolved.cards_set,
            potentials: resolved.potentials,
            atoms: resolved.atoms,
        }
    }

    /// Resolve FEFF/Fortran defaults and inferred run metadata.
    pub fn resolve_fortran_defaults(&self) -> ResolvedFortranInput {
        let cards_set = self.effective_cards_set();
        ResolvedFortranInput {
            edge: self.edge.clone().unwrap_or_else(|| "K".to_string()),
            s02: self.s02,
            control: self.control,
            print_flags: self.print_flags,
            title: self.title.clone(),
            spectroscopy: infer_spectroscopy(&cards_set),
            corehole: infer_corehole_mode(self, &cards_set),
            cards_set,
            potentials: self.potentials.clone(),
            atoms: self.atoms.clone(),
        }
    }

    /// Return cards as a typed stream.
    ///
    /// This is a migration layer from legacy `other_cards` to per-card typed data.
    pub fn typed_cards(&self) -> Vec<TypedCard> {
        let mut out = Vec::with_capacity(self.cards.len());
        for card in &self.cards {
            match card.token.id() {
                7 => {
                    out.push(TypedCard::Title(
                        remainder_after_keyword(&card.line, "TITLE").to_string(),
                    ));
                }
                35 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    let label = words.get(1).copied().unwrap_or("K").to_string();
                    let s02 = parse_f64_word(&words, 2);
                    out.push(TypedCard::Edge { label, s02 });
                }
                41 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    if let Some(v) = parse_f64_word(&words, 1) {
                        out.push(TypedCard::S02(v));
                    } else {
                        out.push(typed_other(card));
                    }
                }
                4 => {
                    let words = split_words(&card.line);
                    match parse_control_print_flags(&words, card.line_num, "CONTROL") {
                        Ok(v) => out.push(TypedCard::Control(v)),
                        Err(_) => out.push(typed_other(card)),
                    }
                }
                13 => {
                    let words = split_words(&card.line);
                    match parse_control_print_flags(&words, card.line_num, "PRINT") {
                        Ok(v) => out.push(TypedCard::Print(v)),
                        Err(_) => out.push(typed_other(card)),
                    }
                }
                24 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    out.push(TypedCard::Exafs(ExafsCard {
                        xkmax: parse_f64_word(&words, 1),
                    }));
                }
                21 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    out.push(TypedCard::Xanes(XanesCard {
                        xkmax: parse_f64_word(&words, 1),
                        xkstep: parse_f64_word(&words, 2),
                        vixan: parse_f64_word(&words, 3),
                    }));
                }
                36 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    out.push(TypedCard::Scf(ScfCard {
                        rfms1: parse_f64_word(&words, 1),
                        lfms1: parse_i32_word(&words, 2),
                        nscmt: parse_i32_word(&words, 3),
                        ca: parse_f64_word(&words, 4),
                        nmix: parse_i32_word(&words, 5),
                    }));
                }
                37 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    out.push(TypedCard::Fms(FmsCard {
                        rfms: parse_f64_word(&words, 1),
                        lfms2: parse_i32_word(&words, 2),
                        minv: parse_i32_word(&words, 3),
                        toler1: parse_f64_word(&words, 4),
                        toler2: parse_f64_word(&words, 5),
                        rdirec: parse_f64_word(&words, 6),
                    }));
                }
                68 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    out.push(TypedCard::Corehole(CoreholeCard {
                        mode: words.get(1).map(|s| s.to_ascii_uppercase()),
                    }));
                }
                5 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    out.push(TypedCard::Exchange(ExchangeCard {
                        ixc: parse_i32_word(&words, 1),
                        vr0: parse_f64_word(&words, 2),
                        vi0: parse_f64_word(&words, 3),
                        ixc0: parse_i32_word(&words, 4),
                    }));
                }
                10 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    out.push(TypedCard::Debye(DebyeCard {
                        temp: parse_f64_word(&words, 1),
                        thetad: parse_f64_word(&words, 2),
                        idwopt: parse_i32_word(&words, 3),
                        extras: words.iter().skip(4).map(|s| (*s).to_string()).collect(),
                    }));
                }
                9 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    out.push(TypedCard::Rpath(RpathCard {
                        rmax: parse_f64_word(&words, 1),
                    }));
                }
                15 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    out.push(TypedCard::Nleg(NlegCard {
                        nleg: parse_i32_word(&words, 1),
                    }));
                }
                65 => {
                    let words = split_words(&card.line);
                    let words = truncate_at_comment_token(&words);
                    out.push(TypedCard::Kmesh(KmeshCard {
                        nkx: parse_i32_word(&words, 1),
                        nky: parse_i32_word(&words, 2),
                        nkz: parse_i32_word(&words, 3),
                        ktype: parse_i32_word(&words, 4),
                        usesym: parse_i32_word(&words, 5),
                    }));
                }
                14 => {
                    let mut potentials = Vec::new();
                    for (i, line) in card.continuation.iter().enumerate() {
                        if let Ok(p) = parse_potential_line_strict(line, card.line_num + i + 1) {
                            potentials.push(p);
                        }
                    }
                    out.push(TypedCard::Potentials(potentials));
                }
                1 => {
                    let mut atoms = Vec::new();
                    for (i, line) in card.continuation.iter().enumerate() {
                        if let Ok(a) = parse_atom_line_strict(line, card.line_num + i + 1) {
                            atoms.push(a);
                        }
                    }
                    out.push(TypedCard::Atoms(atoms));
                }
                _ => out.push(typed_other(card)),
            }
        }
        out
    }

    /// Write feff.inp to a writer.
    pub fn write_to(&self, w: &mut dyn Write) -> std::io::Result<()> {
        if !self.cards.is_empty() {
            return self.write_preserving_cards(w);
        }
        self.write_canonical(w)
    }

    /// Write using a normalized canonical layout.
    pub fn write_canonical(&self, w: &mut dyn Write) -> std::io::Result<()> {
        for title in &self.title {
            writeln!(w, "TITLE {title}")?;
        }

        if let Some(ref edge) = self.edge {
            writeln!(w, "EDGE {edge}")?;
        }
        if let Some(s02) = self.s02 {
            writeln!(w, "S02 {s02}")?;
        }

        writeln!(w)?;
        writeln!(
            w,
            "CONTROL {} {} {} {} {} {}",
            self.control[0],
            self.control[1],
            self.control[2],
            self.control[3],
            self.control[4],
            self.control[5]
        )?;
        writeln!(
            w,
            "PRINT {} {} {} {} {} {}",
            self.print_flags[0],
            self.print_flags[1],
            self.print_flags[2],
            self.print_flags[3],
            self.print_flags[4],
            self.print_flags[5]
        )?;

        // Write other cards, normalizing known problematic cards for ifx compatibility.
        // FEFF10's rdinp.f90 unconditionally reads optional EXCHANGE fields (vr0, vi0)
        // without nwords guards, causing ifx to crash with severe error 24 on blank strings.
        writeln!(w)?;
        for card in &self.other_cards {
            writeln!(w, "{}", normalize_card(card))?;
        }

        // Potentials
        writeln!(w)?;
        writeln!(w, "POTENTIALS")?;
        for pot in &self.potentials {
            write!(w, "{:>5} {:>3}", pot.ipot, pot.z)?;

            let has_tail = pot.l_scmt.is_some() || pot.l_fms.is_some() || pot.stoich.is_some();
            if !pot.tag.is_empty() || has_tail {
                // If optional tail values are present, keep positional meaning by emitting
                // a tag token even when the tag was omitted in the source.
                let tag = if pot.tag.is_empty() && has_tail {
                    "-"
                } else {
                    pot.tag.as_str()
                };
                write!(w, " {:<6}", tag)?;
            }

            if has_tail {
                match pot.l_scmt {
                    Some(l) => write!(w, " {l}")?,
                    None => write!(w, " -1")?,
                }
            }
            if pot.l_fms.is_some() || pot.stoich.is_some() {
                match pot.l_fms {
                    Some(l) => write!(w, " {l}")?,
                    None => write!(w, " -1")?,
                }
            }
            if let Some(stoich) = pot.stoich {
                write!(w, " {stoich}")?;
            }
            writeln!(w)?;
        }

        // Atoms
        writeln!(w)?;
        writeln!(w, "ATOMS")?;
        for atom in &self.atoms {
            writeln!(
                w,
                " {:>10.5} {:>10.5} {:>10.5}  {}    {:<14}{:.5}",
                atom.x, atom.y, atom.z, atom.ipot, atom.tag, atom.distance
            )?;
        }

        writeln!(w)?;
        writeln!(w, "END")?;
        Ok(())
    }

    fn write_preserving_cards(&self, w: &mut dyn Write) -> std::io::Result<()> {
        let mut saw_end = false;
        for card in &self.cards {
            writeln!(w, "{}", normalize_card(&card.line))?;
            if card.token.id() == -1 {
                saw_end = true;
                break;
            }
            for cont in &card.continuation {
                writeln!(w, "{cont}")?;
            }
        }
        if !saw_end {
            writeln!(w, "END")?;
        }
        Ok(())
    }

    /// Validate semantic correctness of the input.
    ///
    /// Checks that potentials, atoms, and cards are consistent and that
    /// FEFF will be able to run without crashing. Call this before running
    /// the pipeline to get clear error messages instead of cryptic Fortran failures.
    ///
    /// Returns `Ok(())` if the input is valid, or `Err(Error::Config(...))` with
    /// all validation errors joined by newlines.
    pub fn validate(&self) -> Result<(), Error> {
        let mut errors = Vec::new();

        // 1. POTENTIALS must not be empty
        if self.potentials.is_empty() {
            errors.push(
                "POTENTIALS section is empty; at least the absorber (ipot=0) is required"
                    .to_string(),
            );
        } else {
            // 2. Absorber potential (ipot=0) must exist
            if !self.potentials.iter().any(|p| p.ipot == 0) {
                errors.push("no absorber potential (ipot=0) defined in POTENTIALS".to_string());
            }

            // 5. No duplicate ipot values
            let mut seen_ipots = HashSet::new();
            for pot in &self.potentials {
                if !seen_ipots.insert(pot.ipot) {
                    errors.push(format!("duplicate potential index ipot={}", pot.ipot));
                }
            }

            // 6. Z in valid range (1-103, up to Lawrencium)
            for pot in &self.potentials {
                if pot.z == 0 || pot.z > 103 {
                    errors.push(format!(
                        "potential ipot={} has Z={} which is outside the valid range 1-103",
                        pot.ipot, pot.z
                    ));
                }
            }
        }

        // 3. ATOMS must not be empty
        if self.atoms.is_empty() {
            errors.push("ATOMS section is empty; at least one atom is required".to_string());
        } else if !self.potentials.is_empty() {
            // 4. All atom ipot values must reference defined potentials
            let valid_ipots: HashSet<u32> = self.potentials.iter().map(|p| p.ipot).collect();
            for (i, atom) in self.atoms.iter().enumerate() {
                if !valid_ipots.contains(&atom.ipot) {
                    errors.push(format!(
                        "atom {} references undefined ipot={}",
                        i, atom.ipot
                    ));
                    break; // avoid spamming hundreds of identical errors
                }
            }

            // 7. At least one atom must reference ipot=0 (absorber site)
            if self.potentials.iter().any(|p| p.ipot == 0)
                && !self.atoms.iter().any(|a| a.ipot == 0)
            {
                errors.push("no atom with ipot=0 (absorber) found in ATOMS list".to_string());
            }
        }

        if let Err(Error::Config(msg)) = self.validate_fortran_rules() {
            for line in msg.lines() {
                if !line.trim().is_empty() {
                    errors.push(line.to_string());
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::Config(errors.join("\n")))
        }
    }

    /// Validate FEFF card-combination rules from `RDINP/consistency.f90`.
    pub fn validate_fortran_rules(&self) -> Result<(), Error> {
        let cards = self.effective_cards_set();
        let has = |id: i16| cards.contains(&id);
        let mut errors = Vec::new();

        // 1) Only one spectroscopy card among XANES/EXAFS/XES/DANES/FPRIME/ELNES/EXELFS.
        let spec_count = [21_i16, 24, 42, 43, 44, 56, 57]
            .iter()
            .filter(|id| has(**id))
            .count();
        if spec_count > 1 {
            errors.push("more than one type of spectroscopy selected".to_string());
        }

        // 2) NRIXS must be combined with exactly one of XANES or EXAFS and no other spectroscopy.
        if has(78) {
            let nrixs_ok = (has(21) as u8 + has(24) as u8) == 1;
            if !nrixs_ok {
                errors.push("NRIXS must be combined with XANES or EXAFS".to_string());
            }
            if has(42) || has(43) || has(44) || has(56) || has(57) {
                errors.push("NRIXS combined with incompatible spectroscopy card".to_string());
            }
        }

        // 3) LJMAX/LDEC only valid with NRIXS and NRIXS incompatible with MULTIPOLE.
        if (has(79) || has(80)) && !has(78) {
            errors.push("LDEC and LJMAX cards only allowed with NRIXS".to_string());
        }
        if has(78) && has(47) {
            errors.push("you cannot combine NRIXS and MULTIPOLE".to_string());
        }

        // 4) Explicitly forbidden with NRIXS.
        if has(78)
            && (has(25)
                || has(26)
                || has(29)
                || has(34)
                || has(40)
                || has(46)
                || has(28)
                || has(42)
                || has(49)
                || has(50)
                || has(104))
        {
            errors.push(
                "NRIXS forbids: ELLIPTICITY, POLARIZATION, NSTAR, SPIN, CFAVERAGE, XNCD/XMCD, RPHASES, XES, TDLDA, PMBSE, HUBBARD"
                    .to_string(),
            );
        }

        // 5) Reciprocal-space requirements.
        if has(62) {
            if !(has(65) && has(71)) {
                errors.push("KMESH and TARGET are required for RECIPROCAL card".to_string());
            }
            let lattice_or_cif = has(64) as u8 + has(92) as u8;
            if lattice_or_cif != 1 {
                errors.push("use either LATTICE or CIF with RECIPROCAL card".to_string());
            }
        }

        // 6) Redundant hole specification.
        if has(30) && has(68) {
            errors.push("Please use only one of the NOHOLE and COREHOLE cards".to_string());
        }

        // 7) No CGRID without COMPTON/RHOZZP.
        if has(96) && !(has(94) || has(95)) {
            errors.push("Cannot use CGRID without COMPTON or RHOZZP".to_string());
        }

        // 8) HUBBARD incompatible with RECIPROCAL.
        if has(104) && has(62) {
            errors.push("Cannot use RECIPROCAL with HUBBARD".to_string());
        }

        // 9) ATOMS and OVERLAP are mutually exclusive (rdinp post-parse fatal check).
        if has(1) && has(3) {
            errors.push("Cannot use ATOMS and OVERLAP in the same feff.inp.".to_string());
        }

        // 10) HOLE card requires ihole > 0.
        if let Some(ihole) = last_card_i32_arg(self, 2, 1)
            && ihole <= 0
        {
            errors.push(
                    "Use NOHOLE to calculate without core hole. Only ihole greater than zero are allowed."
                        .to_string(),
                );
        }

        // 11) MDFF runtime consistency checks tied to NRIXS.
        if let Some(imdff) = last_card_i32_arg(self, 88, 1) {
            if matches!(imdff, 1 | 2) && !has(78) {
                errors.push(
                    "ERROR - the selected MDFF option is only available with the NRIXS card."
                        .to_string(),
                );
            }
            if imdff == 2 && has(78) {
                let nq = last_card_i32_arg(self, 78, 1)
                    .map(normalize_nrixs_nq)
                    .unwrap_or(1);
                if nq != 2 {
                    errors.push(
                        "Current version of this type of MDFF calculation requires that you set nq=2 in the NRIXS card."
                            .to_string(),
                    );
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::Config(errors.join("\n")))
        }
    }
}

fn parse_error(line: usize, message: impl Into<String>) -> Error {
    Error::Parse(ParseError {
        line,
        message: message.into(),
    })
}

fn infer_spectroscopy(cards_set: &HashSet<i16>) -> String {
    let mut s = "EXAFS".to_string();
    if cards_set.contains(&21) {
        s = "XANES".to_string();
    }
    if cards_set.contains(&24) {
        s = "EXAFS".to_string();
    }
    if cards_set.contains(&56) {
        s = "ELNES".to_string();
    }
    if cards_set.contains(&57) {
        s = "EXELFS".to_string();
    }
    if cards_set.contains(&94) {
        s = "COMPTON".to_string();
    }
    if cards_set.contains(&78) {
        s = "NRIXS".to_string();
    }
    if cards_set.contains(&101) {
        s = "RIXS".to_string();
    }
    if cards_set.contains(&42) {
        s = "XES".to_string();
    }
    if cards_set.contains(&44) {
        s = "FPRIME".to_string();
    }
    if cards_set.contains(&46) {
        s = "XMCD".to_string();
    }
    if cards_set.contains(&43) {
        s = "DANES".to_string();
    }
    s
}

fn infer_corehole_mode(input: &FeffInput, cards_set: &HashSet<i16>) -> String {
    let mut mode = "FSR".to_string();

    if cards_set.contains(&30) {
        mode = "no".to_string();
    }

    for card in &input.cards {
        if card.token.id() == 30 {
            let words = split_words(&card.line);
            if let Some(v) = words.get(1).and_then(|s| parse_i32_token(s)) {
                mode = match v {
                    0 => "no".to_string(),
                    2 => "RPA".to_string(),
                    _ => "FSR".to_string(),
                };
            } else {
                mode = "no".to_string();
            }
        }
        if card.token.id() == 68 {
            let words = split_words(&card.line);
            let v = words
                .get(1)
                .map(|s| s.trim().to_ascii_uppercase())
                .unwrap_or_else(|| "FSR".to_string());
            mode = match v.as_str() {
                "NONE" => "no".to_string(),
                "RPA" => "RPA".to_string(),
                "FSR" | "REGULAR" => "FSR".to_string(),
                _ => "FSR".to_string(),
            };
        }
    }

    mode
}

fn last_card_i32_arg(input: &FeffInput, token_id: i16, arg_idx: usize) -> Option<i32> {
    input.cards.iter().rev().find_map(|card| {
        if card.token.id() != token_id {
            return None;
        }
        let words = split_words(&card.line);
        words.get(arg_idx).and_then(|s| parse_i32_token(s))
    })
}

fn normalize_nrixs_nq(raw_nq: i32) -> i32 {
    let mut nq = raw_nq.abs();
    if nq == 0 {
        nq = 1;
    }
    nq
}

fn typed_other(card: &InputCard) -> TypedCard {
    TypedCard::Other {
        token: card.token,
        keyword: card.keyword.clone(),
        line: card.line.clone(),
        continuation: card.continuation.clone(),
    }
}

fn parse_f64_token(token: &str) -> Option<f64> {
    if let Ok(v) = token.parse::<f64>() {
        return Some(v);
    }
    if token.contains('d') || token.contains('D') {
        let normalized = token.replace(['D', 'd'], "e");
        return normalized.parse::<f64>().ok();
    }
    None
}

fn parse_i32_token(token: &str) -> Option<i32> {
    token.parse::<i32>().ok()
}

fn parse_logical_token(token: &str) -> Option<bool> {
    match token.trim().to_ascii_uppercase().as_str() {
        "T" | ".T." | "TRUE" | ".TRUE." => Some(true),
        "F" | ".F." | "FALSE" | ".FALSE." => Some(false),
        _ => None,
    }
}

fn required_word<'a>(
    words: &'a [String],
    idx: usize,
    line_num: usize,
    card: &str,
    field: &str,
) -> Result<&'a str, Error> {
    words
        .get(idx)
        .map(String::as_str)
        .ok_or_else(|| parse_error(line_num, format!("{card} requires {field}")))
}

fn required_i32(
    words: &[String],
    idx: usize,
    line_num: usize,
    card: &str,
    field: &str,
) -> Result<i32, Error> {
    let token = required_word(words, idx, line_num, card, field)?;
    parse_i32_token(token).ok_or_else(|| {
        parse_error(
            line_num,
            format!("{card} {field} is not a valid integer: '{token}'"),
        )
    })
}

fn required_f64(
    words: &[String],
    idx: usize,
    line_num: usize,
    card: &str,
    field: &str,
) -> Result<f64, Error> {
    let token = required_word(words, idx, line_num, card, field)?;
    parse_f64_token(token).ok_or_else(|| {
        parse_error(
            line_num,
            format!("{card} {field} is not a valid number: '{token}'"),
        )
    })
}

fn optional_i32(
    words: &[String],
    idx: usize,
    line_num: usize,
    card: &str,
    field: &str,
) -> Result<(), Error> {
    if let Some(token) = words.get(idx).map(String::as_str)
        && parse_i32_token(token).is_none()
    {
        return Err(parse_error(
            line_num,
            format!("{card} {field} is not a valid integer: '{token}'"),
        ));
    }
    Ok(())
}

fn optional_f64(
    words: &[String],
    idx: usize,
    line_num: usize,
    card: &str,
    field: &str,
) -> Result<(), Error> {
    if let Some(token) = words.get(idx).map(String::as_str)
        && parse_f64_token(token).is_none()
    {
        return Err(parse_error(
            line_num,
            format!("{card} {field} is not a valid number: '{token}'"),
        ));
    }
    Ok(())
}

fn optional_logical(
    words: &[String],
    idx: usize,
    line_num: usize,
    card: &str,
    field: &str,
) -> Result<(), Error> {
    if let Some(token) = words.get(idx).map(String::as_str)
        && parse_logical_token(token).is_none()
    {
        return Err(parse_error(
            line_num,
            format!("{card} {field} is not a valid logical value: '{token}'"),
        ));
    }
    Ok(())
}

fn validate_card_words_fortran(
    token_id: i16,
    words: &[String],
    line_num: usize,
) -> Result<(), Error> {
    let card = CardToken(token_id).canonical_name();
    match token_id {
        1 | 7 | 14 | 17 | 28 | 29 | 32 | 33 | 45 | 46 | 48 | 52 | 53 | 59 | 61 | 62 | 74 | 81
        | 83 | 85 | 89 | 95 | 100 | 102 | 106 | 108 | 110 => {}
        2 => {
            required_i32(words, 1, line_num, card, "ihole")?;
            optional_f64(words, 2, line_num, card, "s02")?;
        }
        3 => {
            required_i32(words, 1, line_num, card, "iph")?;
        }
        4 => {
            let _ = parse_control_print_flags(words, line_num, card)?;
        }
        5 => {
            required_i32(words, 1, line_num, card, "ixc")?;
            optional_f64(words, 2, line_num, card, "vr0")?;
            optional_f64(words, 3, line_num, card, "vi0")?;
            optional_i32(words, 4, line_num, card, "ixc0")?;
        }
        6 => {
            required_i32(words, 1, line_num, card, "ipot")?;
            required_f64(words, 2, line_num, card, "ionization")?;
        }
        8 => {
            required_i32(words, 1, line_num, card, "ipot")?;
            required_f64(words, 2, line_num, card, "folp")?;
        }
        9 => {
            optional_f64(words, 1, line_num, card, "rmax")?;
        }
        10 => {
            required_f64(words, 1, line_num, card, "temp")?;
            required_f64(words, 2, line_num, card, "thetad")?;
            optional_i32(words, 3, line_num, card, "idwopt")?;
            optional_i32(words, 5, line_num, card, "dym_order")?;
            optional_i32(words, 6, line_num, card, "dym_type")?;
            optional_i32(words, 7, line_num, card, "dym_route")?;
        }
        11 => {
            required_f64(words, 1, line_num, card, "rmult")?;
        }
        12 => {
            required_i32(words, 1, line_num, card, "indss")?;
            required_i32(words, 2, line_num, card, "iphss")?;
            required_f64(words, 3, line_num, card, "degss")?;
            required_f64(words, 4, line_num, card, "rss")?;
        }
        13 => {
            let _ = parse_control_print_flags(words, line_num, card)?;
        }
        15 => {
            required_i32(words, 1, line_num, card, "nleg")?;
        }
        16 => {
            required_f64(words, 1, line_num, card, "critcw")?;
            required_f64(words, 2, line_num, card, "critpw")?;
        }
        18 => {
            required_i32(words, 1, line_num, card, "iorder")?;
        }
        19 => {
            required_f64(words, 1, line_num, card, "pcritk")?;
            required_f64(words, 2, line_num, card, "pcrith")?;
        }
        20 => {
            required_f64(words, 1, line_num, card, "sig2")?;
        }
        21 | 42 | 43 | 56 => {
            optional_f64(words, 1, line_num, card, "xkmax")?;
            optional_f64(words, 2, line_num, card, "xkstep")?;
            optional_f64(words, 3, line_num, card, "vixan")?;
        }
        57 => {
            optional_f64(words, 1, line_num, card, "xkmax")?;
        }
        22 => {
            required_f64(words, 1, line_num, card, "vrcorr")?;
            required_f64(words, 2, line_num, card, "vicorr")?;
        }
        23 | 24 => {
            optional_f64(words, 1, line_num, card, "xkmax")?;
        }
        25 => {
            required_f64(words, 1, line_num, card, "x")?;
            required_f64(words, 2, line_num, card, "y")?;
            required_f64(words, 3, line_num, card, "z")?;
        }
        26 => {
            required_f64(words, 1, line_num, card, "elpty")?;
            required_f64(words, 2, line_num, card, "x")?;
            required_f64(words, 3, line_num, card, "y")?;
            required_f64(words, 4, line_num, card, "z")?;
        }
        27 => {
            required_f64(words, 1, line_num, card, "delta")?;
        }
        30 => {
            optional_i32(words, 1, line_num, card, "mode")?;
        }
        31 => {
            required_f64(words, 1, line_num, card, "alphat")?;
            optional_i32(words, 2, line_num, card, "thetae")?;
        }
        34 => {
            required_i32(words, 1, line_num, card, "ispin")?;
            optional_f64(words, 2, line_num, card, "x")?;
            optional_f64(words, 3, line_num, card, "y")?;
            optional_f64(words, 4, line_num, card, "z")?;
        }
        35 => {
            let _ = required_word(words, 1, line_num, card, "edge label")?;
        }
        36 => {
            required_f64(words, 1, line_num, card, "rfms1")?;
            optional_i32(words, 2, line_num, card, "lfms1")?;
            optional_i32(words, 3, line_num, card, "nscmt")?;
            optional_f64(words, 4, line_num, card, "ca")?;
            optional_i32(words, 5, line_num, card, "nmix")?;
            optional_f64(words, 6, line_num, card, "ecv")?;
            optional_i32(words, 7, line_num, card, "icoul")?;
        }
        37 => {
            required_f64(words, 1, line_num, card, "rfms2")?;
            optional_i32(words, 2, line_num, card, "lfms2")?;
            optional_i32(words, 3, line_num, card, "minv")?;
            optional_f64(words, 4, line_num, card, "toler1")?;
            optional_f64(words, 5, line_num, card, "toler2")?;
            optional_f64(words, 6, line_num, card, "rdirec")?;
        }
        38 => {
            required_f64(words, 1, line_num, card, "emin")?;
            required_f64(words, 2, line_num, card, "emax")?;
            required_f64(words, 3, line_num, card, "eimag")?;
            optional_i32(words, 4, line_num, card, "neldos")?;
            optional_i32(words, 5, line_num, card, "ldostype")?;
        }
        39 => {
            required_i32(words, 1, line_num, card, "inters")?;
            optional_f64(words, 2, line_num, card, "totvol")?;
        }
        40 => {
            required_i32(words, 1, line_num, card, "iphabs")?;
            required_i32(words, 2, line_num, card, "nabs")?;
            required_f64(words, 3, line_num, card, "rclabs")?;
        }
        41 => {
            required_f64(words, 1, line_num, card, "s02")?;
        }
        44 => {
            required_f64(words, 1, line_num, card, "emin")?;
            required_f64(words, 2, line_num, card, "emax")?;
            optional_f64(words, 3, line_num, card, "estep")?;
        }
        47 => {
            required_i32(words, 1, line_num, card, "le2")?;
            optional_i32(words, 2, line_num, card, "l2lp")?;
        }
        49 => {
            optional_i32(words, 1, line_num, card, "ifxc")?;
        }
        50 => {
            optional_i32(words, 1, line_num, card, "ipmbse")?;
            optional_i32(words, 2, line_num, card, "nonlocal")?;
            optional_i32(words, 3, line_num, card, "ifxc")?;
            optional_i32(words, 4, line_num, card, "ibasis")?;
        }
        51 => {
            optional_i32(words, 1, line_num, card, "iplsmn")?;
            optional_i32(words, 2, line_num, card, "npoles")?;
        }
        54 => {
            required_f64(words, 1, line_num, card, "wsigk")?;
        }
        55 => {
            required_f64(words, 1, line_num, card, "cen")?;
        }
        58 => {
            required_f64(words, 1, line_num, card, "emagic")?;
        }
        60 => {
            required_i32(words, 1, line_num, card, "icase")?;
        }
        63 => {
            required_i32(words, 1, line_num, card, "igroup")?;
        }
        64 => {
            let _ = required_word(words, 1, line_num, card, "lattice type")?;
            optional_f64(words, 2, line_num, card, "scale")?;
        }
        65 => {
            required_i32(words, 1, line_num, card, "nkx")?;
            if words.len() > 2 {
                required_i32(words, 2, line_num, card, "nky")?;
                required_i32(words, 3, line_num, card, "nkz")?;
            }
            optional_i32(words, 4, line_num, card, "ktype")?;
            optional_i32(words, 5, line_num, card, "usesym")?;
        }
        66 => {
            required_f64(words, 1, line_num, card, "streta")?;
            required_f64(words, 2, line_num, card, "strgmax")?;
            required_f64(words, 3, line_num, card, "strrmax")?;
        }
        67 => {
            if words.len() < 5 {
                return Err(parse_error(
                    line_num,
                    "BANDSTRUCTURE requires at least: emin emax estep ikpath",
                ));
            }
            required_f64(words, 1, line_num, card, "emin")?;
            required_f64(words, 2, line_num, card, "emax")?;
            required_f64(words, 3, line_num, card, "estep")?;
            required_i32(words, 4, line_num, card, "ikpath")?;
            optional_i32(words, 5, line_num, card, "nkp")?;
            optional_logical(words, 6, line_num, card, "freeprop")?;
        }
        68 => {
            if let Some(mode) = words.get(1).map(|s| s.trim().to_ascii_uppercase())
                && !matches!(mode.as_str(), "NONE" | "RPA" | "FSR" | "REGULAR")
            {
                return Err(parse_error(
                    line_num,
                    format!("invalid COREHOLE mode '{mode}'"),
                ));
            }
        }
        71 => {
            required_i32(words, 1, line_num, card, "target index")?;
        }
        72 if words.len() > 1 => {
            let iegrid = required_i32(words, 1, line_num, card, "iegrid")?;
            if iegrid == 2 {
                let _ = required_word(words, 2, line_num, card, "grid filename")?;
            } else if iegrid == 3 {
                required_i32(words, 2, line_num, card, "egrid3a")?;
                required_f64(words, 3, line_num, card, "egrid3b")?;
                required_f64(words, 4, line_num, card, "egrid3c")?;
            }
        }
        73 => {
            let icoord = required_i32(words, 1, line_num, card, "icoord")?;
            if !(1..=6).contains(&icoord) {
                return Err(parse_error(
                    line_num,
                    format!("COORDINATES icoord out of range 1..6: {icoord}"),
                ));
            }
        }
        75 => {
            required_i32(words, 1, line_num, card, "igammach")?;
        }
        76 => {
            required_i32(words, 1, line_num, card, "chshift type")?;
        }
        77 => {
            required_i32(words, 1, line_num, card, "nclusx")?;
            required_i32(words, 2, line_num, card, "lx")?;
        }
        78 => {
            let nq = required_i32(words, 1, line_num, card, "nq")?;
            if nq < 0 {
                required_f64(words, 2, line_num, card, "q")?;
                let q = required_f64(words, 2, line_num, card, "q")?;
                if q <= 0.0 {
                    return Err(parse_error(
                        line_num,
                        "ERROR: momentum transfer negative or zero",
                    ));
                }
                if nq.unsigned_abs() > 1 {
                    required_f64(words, 3, line_num, card, "qweight_re")?;
                    optional_f64(words, 4, line_num, card, "qweight_im")?;
                }
            } else {
                required_f64(words, 2, line_num, card, "qx")?;
                required_f64(words, 3, line_num, card, "qy")?;
                required_f64(words, 4, line_num, card, "qz")?;
                if nq > 1 {
                    required_f64(words, 5, line_num, card, "qweight_re")?;
                    optional_f64(words, 6, line_num, card, "qweight_im")?;
                }
            }
        }
        79 | 80 => {
            required_i32(words, 1, line_num, card, "value")?;
        }
        82 => {
            required_f64(words, 1, line_num, card, "eps0")?;
        }
        84 => {
            required_i32(words, 1, line_num, card, "ipot")?;
            required_f64(words, 2, line_num, card, "numdens")?;
        }
        86 | 87 => {
            required_f64(words, 1, line_num, card, "value")?;
        }
        88 => {
            let imdff = words.get(1).and_then(|s| parse_i32_token(s)).unwrap_or(1);
            if imdff == 2 && words.len() != 2 && words.len() != 4 {
                return Err(parse_error(
                    line_num,
                    "MDFF 2 expects either no extra args or qqmdff cosmdff",
                ));
            }
            optional_i32(words, 1, line_num, card, "imdff")?;
            if words.len() > 2 {
                optional_f64(words, 2, line_num, card, "qqmdff")?;
            }
            if words.len() > 3 {
                optional_f64(words, 3, line_num, card, "cosmdff")?;
            }
        }
        90 if words
            .get(1)
            .map(|s| s.eq_ignore_ascii_case("card"))
            .unwrap_or(false) =>
        {
            required_i32(words, 2, line_num, card, "nlines")?;
        }
        91 => {
            if words.len() < 3 {
                return Err(parse_error(
                    line_num,
                    "SCREEN must be followed by: parameter value",
                ));
            }
            required_f64(words, 2, line_num, card, "value")?;
        }
        92 => {
            let _ = required_word(words, 1, line_num, card, "cif filename")?;
        }
        93 => {
            optional_i32(words, 1, line_num, card, "equivalence mode")?;
        }
        94 => {
            optional_f64(words, 1, line_num, card, "pqmax")?;
            optional_i32(words, 2, line_num, card, "npq")?;
            optional_i32(words, 3, line_num, card, "force_jzzp")?;
        }
        96 => {
            optional_f64(words, 1, line_num, card, "zpmax")?;
            optional_i32(words, 2, line_num, card, "ns")?;
            optional_i32(words, 3, line_num, card, "nphi")?;
            optional_i32(words, 4, line_num, card, "nz")?;
            optional_i32(words, 5, line_num, card, "nzp")?;
        }
        97 | 98 => {
            optional_f64(words, 1, line_num, card, "value")?;
        }
        99 => {
            optional_f64(words, 1, line_num, card, "temperature")?;
            optional_i32(words, 2, line_num, card, "iscfxc")?;
        }
        101 => {
            optional_f64(words, 1, line_num, card, "gam_exp1")?;
            optional_f64(words, 2, line_num, card, "gam_exp2")?;
            optional_f64(words, 3, line_num, card, "xmu")?;
        }
        103 => {
            required_i32(words, 1, line_num, card, "icore")?;
        }
        104 => {
            required_f64(words, 1, line_num, card, "u_hubbard")?;
            required_f64(words, 2, line_num, card, "j_hubbard")?;
            required_f64(words, 3, line_num, card, "fermi_shift")?;
            required_i32(words, 4, line_num, card, "l_hubbard")?;
        }
        105 => {
            required_i32(words, 1, line_num, card, "l_crpa")?;
            required_f64(words, 2, line_num, card, "rcut")?;
        }
        107 => {
            let iscfxc = required_i32(words, 1, line_num, card, "iscfxc")?;
            if !matches!(iscfxc, 11 | 12 | 21 | 22) {
                return Err(parse_error(
                    line_num,
                    format!("SCXC iscfxc must be one of 11, 12, 21, 22; got {iscfxc}"),
                ));
            }
        }
        109 => {
            required_i32(words, 1, line_num, card, "iscfth")?;
            optional_f64(words, 2, line_num, card, "emaxscf")?;
            optional_i32(words, 3, line_num, card, "negrid")?;
            optional_i32(words, 4, line_num, card, "nmu")?;
            optional_f64(words, 5, line_num, card, "xntol")?;
        }
        111 => {
            optional_f64(words, 1, line_num, card, "rfms1_start")?;
            optional_i32(words, 2, line_num, card, "nramp")?;
        }
        112 => {
            let tmp = required_f64(words, 1, line_num, card, "tolmu")?;
            if tmp >= 0.0 {
                optional_f64(words, 2, line_num, card, "tolq")?;
                optional_f64(words, 3, line_num, card, "tolqp")?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn parse_f64_word(words: &[&str], idx: usize) -> Option<f64> {
    words.get(idx).and_then(|s| parse_f64_token(s))
}

fn parse_i32_word(words: &[&str], idx: usize) -> Option<i32> {
    words.get(idx).and_then(|s| s.parse::<i32>().ok())
}

fn parse_potential_line_strict(line: &str, line_num: usize) -> Result<Potential, Error> {
    if line.trim().is_empty() {
        return Err(parse_error(line_num, "empty line in POTENTIALS section"));
    }

    let words = split_words(line);
    let parts: Vec<&str> = words.iter().map(|s| s.as_str()).collect();
    if parts.len() < 2 {
        return Err(parse_error(
            line_num,
            "POTENTIALS line must contain at least: ipot z",
        ));
    }

    let ipot = parts[0].parse::<u32>().map_err(|_| {
        parse_error(
            line_num,
            format!("invalid POTENTIALS ipot value '{}'", parts[0]),
        )
    })?;
    let z = parts[1].parse::<u32>().map_err(|_| {
        parse_error(
            line_num,
            format!("invalid POTENTIALS atomic number '{}'", parts[1]),
        )
    })?;
    let tag = parts.get(2).map(|s| s.to_string()).unwrap_or_default();
    let l_scmt = parts
        .get(3)
        .map(|s| parse_lmax_strict(s, line_num, "l_scmt"))
        .transpose()?
        .flatten();
    let l_fms = parts
        .get(4)
        .map(|s| parse_lmax_strict(s, line_num, "l_fms"))
        .transpose()?
        .flatten();
    let stoich = parts
        .get(5)
        .map(|s| {
            parse_f64_token(s)
                .ok_or_else(|| parse_error(line_num, format!("invalid POTENTIALS stoich '{}'", s)))
        })
        .transpose()?;

    Ok(Potential {
        ipot,
        z,
        tag,
        l_scmt,
        l_fms,
        stoich,
    })
}

fn parse_atom_line_strict(line: &str, line_num: usize) -> Result<Atom, Error> {
    if line.trim().is_empty() {
        return Err(parse_error(line_num, "empty line in ATOMS section"));
    }

    let words = split_words(line);
    let parts: Vec<&str> = words.iter().map(|s| s.as_str()).collect();
    if parts.len() < 4 {
        return Err(parse_error(
            line_num,
            "ATOMS line must contain at least: x y z ipot",
        ));
    }

    let x = parse_f64_token(parts[0]).ok_or_else(|| {
        parse_error(
            line_num,
            format!("invalid atom x coordinate '{}'", parts[0]),
        )
    })?;
    let y = parse_f64_token(parts[1]).ok_or_else(|| {
        parse_error(
            line_num,
            format!("invalid atom y coordinate '{}'", parts[1]),
        )
    })?;
    let z = parse_f64_token(parts[2]).ok_or_else(|| {
        parse_error(
            line_num,
            format!("invalid atom z coordinate '{}'", parts[2]),
        )
    })?;
    let ipot = parts[3]
        .parse::<u32>()
        .map_err(|_| parse_error(line_num, format!("invalid atom ipot '{}'", parts[3])))?;
    // tag is optional (FEFF only requires x y z ipot). If field 5 is numeric,
    // treat it as distance and leave tag empty.
    let (tag, distance) = match parts.get(4) {
        Some(field5) => match parse_f64_token(field5) {
            Some(d) => (String::new(), d),
            None => {
                let d = parts.get(5).and_then(|s| parse_f64_token(s)).unwrap_or(0.0);
                ((*field5).to_string(), d)
            }
        },
        None => (String::new(), 0.0),
    };

    Ok(Atom {
        x,
        y,
        z,
        ipot,
        tag,
        distance,
    })
}

fn parse_overlap_line_strict(line: &str, line_num: usize) -> Result<(), Error> {
    let words = split_words(line);
    if words.len() < 3 {
        return Err(parse_error(
            line_num,
            "OVERLAP line must contain at least: iphovr nnovr rovr",
        ));
    }
    parse_i32_token(&words[0]).ok_or_else(|| {
        parse_error(
            line_num,
            format!("invalid OVERLAP iphovr value '{}'", words[0]),
        )
    })?;
    parse_i32_token(&words[1]).ok_or_else(|| {
        parse_error(
            line_num,
            format!("invalid OVERLAP nnovr value '{}'", words[1]),
        )
    })?;
    parse_f64_token(&words[2]).ok_or_else(|| {
        parse_error(
            line_num,
            format!("invalid OVERLAP rovr value '{}'", words[2]),
        )
    })?;
    Ok(())
}

fn parse_lattice_vector_line_strict(line: &str, line_num: usize) -> Result<(), Error> {
    let words = split_words(line);
    if words.len() < 3 {
        return Err(parse_error(
            line_num,
            "LATTICE vector line must contain 3 numeric values",
        ));
    }
    parse_f64_token(&words[0]).ok_or_else(|| {
        parse_error(
            line_num,
            format!("invalid LATTICE vector x value '{}'", words[0]),
        )
    })?;
    parse_f64_token(&words[1]).ok_or_else(|| {
        parse_error(
            line_num,
            format!("invalid LATTICE vector y value '{}'", words[1]),
        )
    })?;
    parse_f64_token(&words[2]).ok_or_else(|| {
        parse_error(
            line_num,
            format!("invalid LATTICE vector z value '{}'", words[2]),
        )
    })?;
    Ok(())
}

fn parse_nrixs_continuation_line_strict(
    line: &str,
    line_num: usize,
    qaverage: bool,
) -> Result<(), Error> {
    let words = split_words(line);
    if qaverage {
        if words.len() < 2 {
            return Err(parse_error(
                line_num,
                "NRIXS continuation expects: q qweight",
            ));
        }
        let q = parse_f64_token(&words[0]).ok_or_else(|| {
            parse_error(line_num, format!("invalid NRIXS q value '{}'", words[0]))
        })?;
        if q <= 0.0 {
            return Err(parse_error(
                line_num,
                "ERROR: momentum transfer negative or zero",
            ));
        }
        parse_f64_token(&words[1]).ok_or_else(|| {
            parse_error(
                line_num,
                format!("invalid NRIXS qweight value '{}'", words[1]),
            )
        })?;
        if words.len() > 2 {
            parse_f64_token(&words[2]).ok_or_else(|| {
                parse_error(
                    line_num,
                    format!("invalid NRIXS qweight imag value '{}'", words[2]),
                )
            })?;
        }
    } else {
        if words.len() < 4 {
            return Err(parse_error(
                line_num,
                "NRIXS continuation expects: qx qy qz qweight",
            ));
        }
        parse_f64_token(&words[0]).ok_or_else(|| {
            parse_error(line_num, format!("invalid NRIXS qx value '{}'", words[0]))
        })?;
        parse_f64_token(&words[1]).ok_or_else(|| {
            parse_error(line_num, format!("invalid NRIXS qy value '{}'", words[1]))
        })?;
        parse_f64_token(&words[2]).ok_or_else(|| {
            parse_error(line_num, format!("invalid NRIXS qz value '{}'", words[2]))
        })?;
        parse_f64_token(&words[3]).ok_or_else(|| {
            parse_error(
                line_num,
                format!("invalid NRIXS qweight value '{}'", words[3]),
            )
        })?;
        if words.len() > 4 {
            parse_f64_token(&words[4]).ok_or_else(|| {
                parse_error(
                    line_num,
                    format!("invalid NRIXS qweight imag value '{}'", words[4]),
                )
            })?;
        }
    }
    Ok(())
}

fn parse_eels_continuation_line_strict(
    line: &str,
    line_num: usize,
    step: u8,
) -> Result<bool, Error> {
    let words = split_words(line);
    match step {
        5 => {
            // fixlinenow() trims optional fields at the first comment token.
            let words = truncate_at_comment_token(&words);
            let ebeam = words.first().ok_or_else(|| {
                parse_error(line_num, "ELNES/EXELFS continuation line 1 requires ebeam")
            })?;
            parse_f64_token(ebeam).ok_or_else(|| {
                parse_error(
                    line_num,
                    format!("invalid ELNES/EXELFS ebeam value '{ebeam}'"),
                )
            })?;

            let mut skip_orientation = false;
            if let Some(aver_tok) = words.get(1) {
                let aver = parse_i32_token(aver_tok).ok_or_else(|| {
                    parse_error(
                        line_num,
                        format!("invalid ELNES/EXELFS aver value '{aver_tok}'"),
                    )
                })?;
                if aver == 1 {
                    skip_orientation = true;
                }
            }
            if let Some(cross_tok) = words.get(2) {
                parse_i32_token(cross_tok).ok_or_else(|| {
                    parse_error(
                        line_num,
                        format!("invalid ELNES/EXELFS cross value '{cross_tok}'"),
                    )
                })?;
            }
            if let Some(relat_tok) = words.get(3) {
                parse_i32_token(relat_tok).ok_or_else(|| {
                    parse_error(
                        line_num,
                        format!("invalid ELNES/EXELFS relat value '{relat_tok}'"),
                    )
                })?;
            }
            if let Some(iinput_tok) = words.get(4) {
                parse_i32_token(iinput_tok).ok_or_else(|| {
                    parse_error(
                        line_num,
                        format!("invalid ELNES/EXELFS iinput value '{iinput_tok}'"),
                    )
                })?;
            }
            if let Some(spcol_tok) = words.get(5) {
                parse_i32_token(spcol_tok).ok_or_else(|| {
                    parse_error(
                        line_num,
                        format!("invalid ELNES/EXELFS spcol value '{spcol_tok}'"),
                    )
                })?;
            }
            Ok(skip_orientation)
        }
        4 => {
            if words.len() < 3 {
                return Err(parse_error(
                    line_num,
                    "ELNES/EXELFS continuation line 2 requires: x y z",
                ));
            }
            for (axis, tok) in [("x", &words[0]), ("y", &words[1]), ("z", &words[2])] {
                parse_f64_token(tok).ok_or_else(|| {
                    parse_error(
                        line_num,
                        format!("invalid ELNES/EXELFS beam {axis} value '{tok}'"),
                    )
                })?;
            }
            Ok(false)
        }
        3 => {
            if words.len() < 2 {
                return Err(parse_error(
                    line_num,
                    "ELNES/EXELFS continuation line 3 requires: acoll aconv",
                ));
            }
            parse_f64_token(&words[0]).ok_or_else(|| {
                parse_error(
                    line_num,
                    format!("invalid ELNES/EXELFS acoll value '{}'", words[0]),
                )
            })?;
            parse_f64_token(&words[1]).ok_or_else(|| {
                parse_error(
                    line_num,
                    format!("invalid ELNES/EXELFS aconv value '{}'", words[1]),
                )
            })?;
            Ok(false)
        }
        2 => {
            if words.len() < 2 {
                return Err(parse_error(
                    line_num,
                    "ELNES/EXELFS continuation line 4 requires: nqr nqf",
                ));
            }
            parse_i32_token(&words[0]).ok_or_else(|| {
                parse_error(
                    line_num,
                    format!("invalid ELNES/EXELFS nqr value '{}'", words[0]),
                )
            })?;
            parse_i32_token(&words[1]).ok_or_else(|| {
                parse_error(
                    line_num,
                    format!("invalid ELNES/EXELFS nqf value '{}'", words[1]),
                )
            })?;
            Ok(false)
        }
        1 => {
            if words.len() < 2 {
                return Err(parse_error(
                    line_num,
                    "ELNES/EXELFS continuation line 5 requires: thetax thetay",
                ));
            }
            parse_f64_token(&words[0]).ok_or_else(|| {
                parse_error(
                    line_num,
                    format!("invalid ELNES/EXELFS thetax value '{}'", words[0]),
                )
            })?;
            parse_f64_token(&words[1]).ok_or_else(|| {
                parse_error(
                    line_num,
                    format!("invalid ELNES/EXELFS thetay value '{}'", words[1]),
                )
            })?;
            Ok(false)
        }
        _ => Err(parse_error(
            line_num,
            format!("invalid ELNES/EXELFS continuation state: {step}"),
        )),
    }
}

fn parse_lmax_strict(token: &str, line_num: usize, field: &str) -> Result<Option<u32>, Error> {
    let v = token
        .parse::<i32>()
        .map_err(|_| parse_error(line_num, format!("invalid POTENTIALS {field} '{token}'")))?;
    if v < 0 {
        // In FEFF, negative lmax means "use default based on Z".
        Ok(None)
    } else {
        Ok(Some(v as u32))
    }
}

fn parse_control_print_flags(words: &[String], line: usize, card: &str) -> Result<[u32; 6], Error> {
    let words: Vec<&str> = words.iter().map(|s| s.as_str()).collect();

    // In FEFF7 files: CONTROL/PRINT are 4 args after keyword: a b c d => a a a b c d
    // In FEFF8+ files: 6 args after keyword: a b c d e f
    if words.len() == 5 {
        let mut vals = [0_u32; 4];
        for (i, token) in words[1..5].iter().enumerate() {
            vals[i] = token.parse::<u32>().map_err(|_| {
                parse_error(
                    line,
                    format!("{card} value {} is not a valid integer: '{token}'", i + 1),
                )
            })?;
        }
        return Ok([vals[0], vals[0], vals[0], vals[1], vals[2], vals[3]]);
    }
    if words.len() < 7 {
        return Err(parse_error(
            line,
            format!("{card} must provide either 4 or 6 values"),
        ));
    }

    let mut out = [0_u32; 6];
    for (i, token) in words[1..7].iter().enumerate() {
        out[i] = token.parse::<u32>().map_err(|_| {
            parse_error(
                line,
                format!("{card} value {} is not a valid integer: '{token}'", i + 1),
            )
        })?;
    }
    Ok(out)
}

fn collect_source_lines_from_file(path: &Path) -> Result<Vec<SourceLine>, Error> {
    let mut out = Vec::new();
    let mut stack = Vec::new();
    collect_source_lines_recursive(path, &mut stack, &mut out)?;
    Ok(out)
}

fn collect_source_lines_from_content(
    content: &str,
    base_dir: Option<PathBuf>,
) -> Result<Vec<SourceLine>, Error> {
    let mut out = Vec::new();
    let mut stack = Vec::new();
    collect_source_lines_text(content, base_dir.as_deref(), &mut stack, &mut out)?;
    Ok(out)
}

fn collect_source_lines_recursive(
    path: &Path,
    stack: &mut Vec<PathBuf>,
    out: &mut Vec<SourceLine>,
) -> Result<(), Error> {
    if stack.len() >= 10 {
        return Err(Error::Config(
            "too many nested include/load directives (max 10)".to_string(),
        ));
    }
    let canonical = std::fs::canonicalize(path).map_err(Error::Io)?;
    if stack.contains(&canonical) {
        return Err(Error::Config(format!(
            "recursive include/load detected for '{}'",
            canonical.display()
        )));
    }
    stack.push(canonical.clone());
    let content = std::fs::read_to_string(&canonical)?;
    let base_dir = canonical.parent().map(Path::to_path_buf);
    collect_source_lines_text(&content, base_dir.as_deref(), stack, out)?;
    stack.pop();
    Ok(())
}

fn collect_source_lines_text(
    content: &str,
    base_dir: Option<&Path>,
    stack: &mut Vec<PathBuf>,
    out: &mut Vec<SourceLine>,
) -> Result<(), Error> {
    for (idx, raw) in content.lines().enumerate() {
        let line_num = idx + 1;
        let line = untab(raw).trim_start().to_string();
        if is_comment_or_blank(&line) {
            continue;
        }
        let words = split_words(&line);
        if words.len() >= 2
            && (words[0].eq_ignore_ascii_case("include") || words[0].eq_ignore_ascii_case("load"))
        {
            let include_name = parse_include_filename(&words[1])
                .ok_or_else(|| parse_error(line_num, "cannot determine include/load filename"))?;
            let include_path = resolve_include_path(&include_name, base_dir);
            collect_source_lines_recursive(&include_path, stack, out)?;
            continue;
        }
        out.push(SourceLine { line, line_num });
    }
    Ok(())
}

fn parse_include_filename(token: &str) -> Option<String> {
    let t = token.trim();
    if t.is_empty() {
        return None;
    }
    let pairs = [
        ('"', '"'),
        ('{', '}'),
        ('(', ')'),
        ('<', '>'),
        ('\'', '\''),
        ('[', ']'),
    ];
    let first = t.chars().next()?;
    for (open, close) in pairs {
        if first == open {
            let mut chars = t.chars();
            chars.next();
            let rest = chars.collect::<String>();
            if let Some(pos) = rest.find(close) {
                return Some(rest[..pos].to_string());
            }
            return None;
        }
    }
    Some(t.to_string())
}

fn resolve_include_path(name: &str, base_dir: Option<&Path>) -> PathBuf {
    let p = PathBuf::from(name);
    if p.is_absolute() {
        p
    } else if let Some(base) = base_dir {
        base.join(p)
    } else {
        p
    }
}

fn untab(line: &str) -> String {
    line.replace('\t', " ")
}

fn is_comment_or_blank(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return true;
    }
    matches!(trimmed.chars().next(), Some(';' | '*' | '%' | '#'))
}

fn remainder_after_keyword<'a>(line: &'a str, first_word: &str) -> &'a str {
    line[first_word.len()..].trim_start()
}

fn split_words(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut between = true;
    let mut comma_found = true;

    for ch in line.chars() {
        if ch == ' ' || ch == '\t' {
            if !between {
                out.push(current.clone());
                current.clear();
                between = true;
                comma_found = false;
            }
            continue;
        }
        if ch == ',' {
            if !between {
                out.push(current.clone());
                current.clear();
                between = true;
            } else if comma_found {
                out.push(String::new());
            }
            comma_found = true;
            continue;
        }
        if between {
            between = false;
        }
        current.push(ch);
    }

    if !between {
        out.push(current);
    }
    out
}

fn truncate_at_comment_token(words: &[String]) -> Vec<&str> {
    let mut out = Vec::with_capacity(words.len());
    for word in words {
        if word
            .chars()
            .next()
            .is_some_and(|c| matches!(c, ';' | '*' | '%' | '#'))
        {
            break;
        }
        out.push(word.as_str());
    }
    out
}

/// Normalize cards that have known ifx-incompatible optional fields in FEFF10's rdinp.f90.
///
/// EXCHANGE: rdinp unconditionally reads vr0 (word 3) and vi0 (word 4) even when only
/// ixc is provided. With gfortran, blank strings read as 0.0; with ifx, they crash
/// (severe error 24). We pad missing fields with the documented defaults.
fn normalize_card(card: &str) -> String {
    let parts: Vec<&str> = card.split_whitespace().collect();
    if parts.is_empty() {
        return card.to_string();
    }
    match parts[0].to_uppercase().as_str() {
        "EXCHANGE" => {
            // EXCHANGE ixc [vr0] [vi0] [ixc0]
            // Defaults: vr0=0.0, vi0=0.0
            let ixc = parts.get(1).copied().unwrap_or("0");
            let vr0 = parts.get(2).copied().unwrap_or("0.0");
            let vi0 = parts.get(3).copied().unwrap_or("0.0");
            if let Some(ixc0) = parts.get(4) {
                format!("EXCHANGE {ixc} {vr0} {vi0} {ixc0}")
            } else {
                format!("EXCHANGE {ixc} {vr0} {vi0}")
            }
        }
        _ => card.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_submodule() -> bool {
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../feff10/examples/EXAFS/Cu/feff.inp"
        ))
        .exists()
    }

    #[test]
    fn parse_exafs_cu() {
        if !has_submodule() {
            return;
        }
        let content = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../feff10/examples/EXAFS/Cu/feff.inp"
        ))
        .unwrap();
        let input = FeffInput::parse(&content).unwrap();

        assert_eq!(input.title.len(), 1);
        assert!(input.title[0].contains("Cu crystal"));
        assert_eq!(input.edge.as_deref(), Some("K"));
        assert_eq!(input.s02, Some(1.0));
        assert_eq!(input.control, [1, 1, 1, 1, 1, 1]);
        assert_eq!(input.potentials.len(), 2);
        assert_eq!(input.potentials[0].z, 29);
        assert_eq!(input.atoms.len(), 79);
    }

    #[test]
    fn round_trip() {
        if !has_submodule() {
            return;
        }
        let content = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../feff10/examples/EXAFS/Cu/feff.inp"
        ))
        .unwrap();
        let input = FeffInput::parse(&content).unwrap();

        let mut buf = Vec::new();
        input.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // Parse the output again - should produce same structure
        let reparsed = FeffInput::parse(&output).unwrap();
        assert_eq!(input.potentials.len(), reparsed.potentials.len());
        assert_eq!(input.atoms.len(), reparsed.atoms.len());
        assert_eq!(input.edge, reparsed.edge);
    }

    #[test]
    fn parse_strict_exafs_cu() {
        if !has_submodule() {
            return;
        }
        let content = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../feff10/examples/EXAFS/Cu/feff.inp"
        ))
        .unwrap();
        let input = FeffInput::parse_strict(&content).unwrap();
        assert_eq!(input.control, [1, 1, 1, 1, 1, 1]);
    }

    #[test]
    fn strict_rejects_short_control() {
        let content = "\
TITLE test
CONTROL 1 1 1
PRINT 0 0 0 0 0 0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";

        let err = FeffInput::parse_strict(content).unwrap_err();
        match err {
            Error::Parse(e) => {
                assert_eq!(e.line, 2);
                assert!(e.message.contains("CONTROL"));
            }
            _ => panic!("expected parse error"),
        }
    }

    #[test]
    fn strict_rejects_bad_potential_line() {
        let content = "\
TITLE test
CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
POTENTIALS
0 xx Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";

        let err = FeffInput::parse_strict(content).unwrap_err();
        match err {
            Error::Parse(e) => {
                assert_eq!(e.line, 5);
                assert!(e.message.contains("POTENTIALS"));
            }
            _ => panic!("expected parse error"),
        }
    }

    #[test]
    fn strict_rejects_bad_atom_line() {
        let content = "\
TITLE test
CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 bad 0 Cu
END
";

        let err = FeffInput::parse_strict(content).unwrap_err();
        match err {
            Error::Parse(e) => {
                assert_eq!(e.line, 7);
                assert!(e.message.contains("coordinate"));
            }
            _ => panic!("expected parse error"),
        }
    }

    #[test]
    fn parse_empty_input() {
        let input = FeffInput::parse("").unwrap();
        assert!(input.title.is_empty());
        assert!(input.edge.is_none());
        assert!(input.s02.is_none());
        assert!(input.potentials.is_empty());
        assert!(input.atoms.is_empty());
    }

    #[test]
    fn parse_minimal_input() {
        let content = "\
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.potentials.len(), 1);
        assert_eq!(input.atoms.len(), 1);
        // Default control when not specified
        assert_eq!(input.control, [1; 6]);
    }

    #[test]
    fn default_has_all_control_ones() {
        let input = FeffInput::default();
        assert_eq!(input.control, [1, 1, 1, 1, 1, 1]);
        assert_eq!(input.print_flags, [0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn parse_preserves_other_cards() {
        let content = "\
TITLE test
EDGE K
EXAFS 20.0
RPATH 5.5
SCF 5.0
DEBYE 300 500
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        let cards_upper: Vec<String> = input.other_cards.iter().map(|c| c.to_uppercase()).collect();
        assert!(
            cards_upper.iter().any(|c| c.starts_with("EXAFS")),
            "EXAFS card not preserved"
        );
        assert!(
            cards_upper.iter().any(|c| c.starts_with("RPATH")),
            "RPATH card not preserved"
        );
        assert!(
            cards_upper.iter().any(|c| c.starts_with("SCF")),
            "SCF card not preserved"
        );
        assert!(
            cards_upper.iter().any(|c| c.starts_with("DEBYE")),
            "DEBYE card not preserved"
        );
    }

    #[test]
    fn typed_cards_parses_common_numeric_cards() {
        let content = "\
TITLE typed
EDGE L3
S02 0.9
EXAFS 20.0
SCF 4.0 1 30 0.2 5
FMS 8.0 1 0 0.001 0.002 4
COREHOLE RPA
EXCHANGE 0 0.1 0.2 2
DEBYE 300 500 0 extras
RPATH 5.5
NLEG 6
KMESH 4 4 4 0 1
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        let typed = input.typed_cards();

        let xanes_only = "\
TITLE typed xanes
XANES 8.0 0.05 0.3
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let typed_xanes = FeffInput::parse(xanes_only).unwrap().typed_cards();

        assert!(typed.iter().any(|c| matches!(
            c,
            TypedCard::Exafs(ExafsCard { xkmax: Some(v) }) if (*v - 20.0).abs() < 1.0e-12
        )));
        assert!(typed_xanes.iter().any(|c| matches!(
            c,
            TypedCard::Xanes(XanesCard {
                xkmax: Some(8.0),
                xkstep: Some(0.05),
                vixan: Some(0.3),
            })
        )));
        assert!(typed.iter().any(|c| matches!(
            c,
            TypedCard::Scf(ScfCard {
                rfms1: Some(4.0),
                lfms1: Some(1),
                nscmt: Some(30),
                ca: Some(0.2),
                nmix: Some(5),
            })
        )));
        assert!(typed.iter().any(|c| matches!(
            c,
            TypedCard::Fms(FmsCard {
                rfms: Some(8.0),
                lfms2: Some(1),
                minv: Some(0),
                toler1: Some(0.001),
                toler2: Some(0.002),
                rdirec: Some(4.0),
            })
        )));
        assert!(typed.iter().any(|c| matches!(
            c,
            TypedCard::Corehole(CoreholeCard { mode: Some(mode) }) if mode == "RPA"
        )));
        assert!(typed.iter().any(|c| matches!(
            c,
            TypedCard::Exchange(ExchangeCard {
                ixc: Some(0),
                vr0: Some(0.1),
                vi0: Some(0.2),
                ixc0: Some(2),
            })
        )));
        assert!(typed.iter().any(|c| matches!(
            c,
            TypedCard::Debye(DebyeCard {
                temp: Some(300.0),
                thetad: Some(500.0),
                idwopt: Some(0),
                extras,
            }) if extras == &vec!["extras".to_string()]
        )));
        assert!(
            typed
                .iter()
                .any(|c| matches!(c, TypedCard::Rpath(RpathCard { rmax: Some(5.5) })))
        );
        assert!(
            typed
                .iter()
                .any(|c| matches!(c, TypedCard::Nleg(NlegCard { nleg: Some(6) })))
        );
        assert!(typed.iter().any(|c| matches!(
            c,
            TypedCard::Kmesh(KmeshCard {
                nkx: Some(4),
                nky: Some(4),
                nkz: Some(4),
                ktype: Some(0),
                usesym: Some(1),
            })
        )));
    }

    #[test]
    fn parse_comments_and_blank_lines_ignored() {
        let content = "\
* This is a comment
TITLE test

* another comment
EDGE K
POTENTIALS
* potential comments
0 29 Cu
ATOMS
* atom comments
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.edge.as_deref(), Some("K"));
        assert_eq!(input.potentials.len(), 1);
        assert_eq!(input.atoms.len(), 1);
    }

    #[test]
    fn parse_fortran_comment_prefixes_ignored() {
        let content = "\
; semicolon comment
# hash comment
% percent comment
   * star comment with leading spaces
TITLE test
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.title, vec!["test".to_string()]);
        assert_eq!(input.potentials.len(), 1);
        assert_eq!(input.atoms.len(), 1);
    }

    #[test]
    fn parse_rejects_unknown_keyword() {
        let content = "\
TITLE test
FOOBAR 123
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err();
        match err {
            Error::Parse(e) => {
                assert_eq!(e.line, 2);
                assert!(e.message.contains("unrecognized keyword"));
            }
            _ => panic!("expected parse error"),
        }
    }

    #[test]
    fn parse_rejects_rconv_keyword_like_fortran_itoken() {
        let content = "\
TITLE test
RCONV 0.0 cf.bin
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(err.contains("unrecognized keyword"), "{err}");
    }

    #[test]
    fn parse_rejects_non_integer_chshift_like_fortran() {
        let content = "\
TITLE test
CHSHIFT 1.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(err.contains("CHSHIFT"), "{err}");
        assert!(err.contains("integer"), "{err}");
    }

    #[test]
    fn parse_accepts_bandstructure_with_logical_freeprop() {
        let content = "\
TITLE test
BANDSTRUCTURE -5.0 5.0 0.5 1 20 T
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        FeffInput::parse(content).unwrap();
    }

    #[test]
    fn parse_rejects_bandstructure_with_numeric_freeprop_like_fortran() {
        let content = "\
TITLE test
BANDSTRUCTURE -5.0 5.0 0.5 1 20 1.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(err.contains("BANDSTRUCTURE"), "{err}");
        assert!(err.contains("logical"), "{err}");
    }

    #[test]
    fn parse_rejects_multiple_spectroscopy_cards() {
        let content = "\
TITLE test
XANES 8.0 0.05
EXAFS 20.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(err.contains("more than one type of spectroscopy"), "{err}");
    }

    #[test]
    fn parse_rejects_reciprocal_without_kmesh_and_target() {
        let content = "\
TITLE test
RECIPROCAL
LATTICE FCC 1.0
1.0 0.0 0.0
0.0 1.0 0.0
0.0 0.0 1.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(err.contains("KMESH and TARGET"), "{err}");
    }

    #[test]
    fn parse_supports_nrixs_multiline_q_list() {
        let content = "\
TITLE nrixs
XANES 8.0 0.05
NRIXS 2 0.0 0.0 1.0 1.0
0.0 1.0 0.0 1.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        let nrixs = input
            .cards
            .iter()
            .find(|c| c.token.id() == 78)
            .expect("NRIXS card not captured");
        assert_eq!(nrixs.continuation.len(), 1);
        assert!(nrixs.continuation[0].contains("0.0 1.0 0.0 1.0"));
    }

    #[test]
    fn parse_accepts_nrixs_qaverage_single_q_without_weight() {
        let content = "\
TITLE nrixs avg
XANES 8.0 0.05
NRIXS -1 24.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        let nrixs = input
            .cards
            .iter()
            .find(|c| c.token.id() == 78)
            .expect("NRIXS card not captured");
        assert!(nrixs.continuation.is_empty());
    }

    #[test]
    fn parse_rejects_nrixs_qaverage_with_non_positive_q() {
        let content = "\
TITLE nrixs avg
XANES 8.0 0.05
NRIXS -1 0.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(err.contains("momentum transfer negative or zero"), "{err}");
    }

    #[test]
    fn parse_rejects_nrixs_qaverage_continuation_with_non_positive_q() {
        let content = "\
TITLE nrixs avg
XANES 8.0 0.05
NRIXS -2 24.0 1.0
0.0 1.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(err.contains("momentum transfer negative or zero"), "{err}");
    }

    #[test]
    fn parse_supports_config_card_block() {
        let content = "\
TITLE cfg
CONFIG card 2
first config line
second config line
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        let config = input
            .cards
            .iter()
            .find(|c| c.token.id() == 90)
            .expect("CONFIG card not captured");
        assert_eq!(config.continuation.len(), 2);
        assert_eq!(config.continuation[0], "first config line");
        assert_eq!(config.continuation[1], "second config line");
    }

    #[test]
    fn parse_supports_elnes_continuation_with_averaging() {
        let content = "\
TITLE elnes
ELNES 8.0
200 1 0 0 0 0
10 20
30 40
50 60
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        let elnes = input
            .cards
            .iter()
            .find(|c| c.token.id() == 56)
            .expect("ELNES card not captured");
        assert_eq!(elnes.continuation.len(), 4);
    }

    #[test]
    fn parse_supports_elnes_first_continuation_fixlinenow_comments() {
        let content = "\
TITLE elnes
ELNES 8.0
200 ; comment
1 0 0
10 20
30 40
50 60
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        let elnes = input
            .cards
            .iter()
            .find(|c| c.token.id() == 56)
            .expect("ELNES card not captured");
        assert_eq!(elnes.continuation.len(), 5);
    }

    #[test]
    fn parse_from_file_supports_include_and_load() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("extras.inp"),
            "\
EDGE K
S02 1.0
CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
EXAFS 20.0
",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("feff.inp"),
            "\
TITLE include test
include extras.inp
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
",
        )
        .unwrap();

        let input = FeffInput::from_file(dir.path().join("feff.inp")).unwrap();
        assert_eq!(input.edge.as_deref(), Some("K"));
        assert_eq!(input.s02, Some(1.0));
        assert_eq!(input.control, [1, 1, 1, 1, 1, 1]);
        assert_eq!(input.potentials.len(), 1);
        assert_eq!(input.atoms.len(), 1);
    }

    #[test]
    fn parse_from_file_rejects_recursive_include() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("a.inp"),
            "\
include b.inp
TITLE rec
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("b.inp"),
            "\
load a.inp
",
        )
        .unwrap();

        let err = FeffInput::from_file(dir.path().join("a.inp")).unwrap_err();
        match err {
            Error::Config(msg) => assert!(msg.contains("recursive include/load")),
            _ => panic!("expected config error"),
        }
    }

    #[test]
    fn parse_inline_comment_in_potentials_is_rejected_like_fortran() {
        let content = "\
POTENTIALS
0 29 Cu * absorber
1 26 Fe * scatterer
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err();
        match err {
            Error::Parse(e) => {
                assert_eq!(e.line, 2);
                assert!(e.message.contains("l_scmt"));
            }
            _ => panic!("expected parse error"),
        }
    }

    #[test]
    fn parse_edge_and_s02() {
        let content = "\
EDGE L3
S02 0.9
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.edge.as_deref(), Some("L3"));
        assert_eq!(input.s02, Some(0.9));
    }

    #[test]
    fn parse_edge_and_s02_with_inline_comments() {
        let content = "\
EDGE K * edge comment
S02 0.9 * amplitude reduction
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.edge.as_deref(), Some("K"));
        assert_eq!(input.s02, Some(0.9));
    }

    #[test]
    fn parse_edge_uses_first_token_only() {
        let content = "\
EDGE L3 VAL
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.edge.as_deref(), Some("L3"));
    }

    #[test]
    fn parse_multiple_titles() {
        let content = "\
TITLE Line one
TITLE Line two
TITLE Line three
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.title.len(), 3);
        assert_eq!(input.title[0], "Line one");
        assert_eq!(input.title[2], "Line three");
    }

    #[test]
    fn write_and_reparse_preserves_structure() {
        let content = "\
TITLE Test compound
EDGE K
S02 0.85
CONTROL 1 1 1 0 0 0
PRINT 0 0 0 0 0 0
EXAFS 20.0
POTENTIALS
0 29 Cu
1 26 Fe
ATOMS
0.000 0.000 0.000 0 Cu 0.00000
1.805 1.805 0.000 1 Fe 2.55270
END
";
        let input = FeffInput::parse(content).unwrap();
        let mut buf = Vec::new();
        input.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let reparsed = FeffInput::parse(&output).unwrap();

        assert_eq!(input.title, reparsed.title);
        assert_eq!(input.edge, reparsed.edge);
        assert_eq!(input.s02, reparsed.s02);
        assert_eq!(input.control, reparsed.control);
        assert_eq!(input.print_flags, reparsed.print_flags);
        assert_eq!(input.potentials.len(), reparsed.potentials.len());
        assert_eq!(input.atoms.len(), reparsed.atoms.len());
        for (a, b) in input.potentials.iter().zip(reparsed.potentials.iter()) {
            assert_eq!(a.ipot, b.ipot);
            assert_eq!(a.z, b.z);
        }
    }

    #[test]
    fn strict_rejects_bad_s02() {
        let content = "\
S02 not_a_number
CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse_strict(content).unwrap_err();
        match err {
            Error::Parse(e) => assert!(e.message.contains("S02")),
            _ => panic!("expected parse error"),
        }
    }

    #[test]
    fn parse_potential_with_optional_fields() {
        let content = "\
POTENTIALS
0 29 Cu 2 3 1.5
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.potentials[0].l_scmt, Some(2));
        assert_eq!(input.potentials[0].l_fms, Some(3));
        assert_eq!(input.potentials[0].stoich, Some(1.5));
    }

    #[test]
    fn parse_potential_tag_is_optional() {
        let content = "\
POTENTIALS
0 29
ATOMS
0.0 0.0 0.0 0
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.potentials.len(), 1);
        assert_eq!(input.potentials[0].ipot, 0);
        assert_eq!(input.potentials[0].z, 29);
        assert_eq!(input.potentials[0].tag, "");
    }

    #[test]
    fn strict_potential_tag_is_optional() {
        let content = "\
POTENTIALS
0 29
ATOMS
0.0 0.0 0.0 0
END
";
        let input = FeffInput::parse_strict(content).unwrap();
        assert_eq!(input.potentials[0].tag, "");
    }

    #[test]
    fn parse_negative_lmax_means_default() {
        let content = "\
POTENTIALS
0 29 Cu -1 -1 1.0
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse_strict(content).unwrap();
        assert_eq!(input.potentials[0].l_scmt, None);
        assert_eq!(input.potentials[0].l_fms, None);
        assert_eq!(input.potentials[0].stoich, Some(1.0));
    }

    #[test]
    fn parse_atom_with_distance() {
        let content = "\
POTENTIALS
0 29 Cu
ATOMS
1.805 1.805 0.000 0 Cu 2.55270
END
";
        let input = FeffInput::parse(content).unwrap();
        assert!((input.atoms[0].x - 1.805).abs() < 1e-10);
        assert!((input.atoms[0].distance - 2.55270).abs() < 1e-4);
    }

    #[test]
    fn parse_atom_distance_without_tag() {
        let content = "\
POTENTIALS
0 29 Cu
ATOMS
1.805 1.805 0.000 0 2.55270
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.atoms[0].tag, "");
        assert!((input.atoms[0].distance - 2.55270).abs() < 1e-4);
    }

    #[test]
    fn parse_control_and_print_feff7_shorthand() {
        let content = "\
CONTROL 1 0 1 0
PRINT 2 3 4 5
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.control, [1, 1, 1, 0, 1, 0]);
        assert_eq!(input.print_flags, [2, 2, 2, 3, 4, 5]);
    }

    #[test]
    fn strict_accepts_control_and_print_feff7_shorthand() {
        let content = "\
CONTROL 1 0 1 0
PRINT 2 3 4 5
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse_strict(content).unwrap();
        assert_eq!(input.control, [1, 1, 1, 0, 1, 0]);
        assert_eq!(input.print_flags, [2, 2, 2, 3, 4, 5]);
    }

    #[test]
    fn write_preserves_potential_stoich() {
        let content = "\
POTENTIALS
0 29 Cu 2 3 1.5
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        let mut buf = Vec::new();
        input.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let reparsed = FeffInput::parse(&output).unwrap();
        assert_eq!(reparsed.potentials[0].stoich, Some(1.5));
    }

    #[test]
    fn parse_all_bundled_examples() {
        if !has_submodule() {
            return;
        }
        let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../feff10/examples");
        let dirs = ["EXAFS/Cu", "EXAFS/SF6", "XANES/Cu", "XANES/BN", "XES/Cu"];
        for d in dirs {
            let path = format!("{examples_dir}/{d}/feff.inp");
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
            let input = FeffInput::parse(&content)
                .unwrap_or_else(|e| panic!("failed to parse {path}: {e}"));
            assert!(
                !input.potentials.is_empty(),
                "{d}/feff.inp has no potentials"
            );
            assert!(!input.atoms.is_empty(), "{d}/feff.inp has no atoms");
        }
    }

    #[test]
    fn roundtrip_xraylarch_cif2feff_input() {
        // Real-world input generated by xraylarch cif2feff (Pt L3-edge)
        let content = "\
*** feff input generated by xraylarch cif2feff using pymatgen ***
TITLE Structure from AMCSD, AMS_ID: 11157
TITLE Mineral Name: Platinum
TITLE Formula:    Pt
TITLE SpaceGroup: Fm-3m
* crystallographics sites: note that these sites may not be unique!
*     using absorber at site 1 in the list below

EDGE    L3
S02     1.0
CONTROL 1 1 1 1 1 1
PRINT   1 0 0 0 0 3
EXAFS   20.0
NLEG     6
RPATH   7.00
*SCF    5.0

EXCHANGE 0

*  POLARIZATION  0 0 0

POTENTIALS
*    IPOT  Z   Tag
      0    78   Pt
      1    78   Pt

ATOMS
*    x         y         z       ipot  tag   distance  site_info
    0.00000   0.00000   0.00000    0   Pt    0.00000  * Pt_1
   -1.96155  -1.96155  -0.00000    1   Pt    2.77405  * Pt_1
    1.96155   1.96155   0.00000    1   Pt    2.77405  * Pt_1

* END%
";
        let inp = FeffInput::parse(content).unwrap();

        // Verify parsing
        assert_eq!(inp.title.len(), 4);
        assert_eq!(inp.edge.as_deref(), Some("L3"));
        assert_eq!(inp.s02, Some(1.0));
        assert_eq!(inp.control, [1, 1, 1, 1, 1, 1]);
        assert_eq!(inp.print_flags, [1, 0, 0, 0, 0, 3]);
        assert_eq!(inp.potentials.len(), 2);
        assert_eq!(inp.potentials[0].z, 78);
        assert_eq!(inp.atoms.len(), 3);
        assert_eq!(inp.atoms[0].ipot, 0);
        assert_eq!(inp.atoms[1].ipot, 1);

        // Verify other_cards preserved EXAFS, NLEG, RPATH, EXCHANGE
        assert!(inp.other_cards.iter().any(|c| c.starts_with("EXAFS")));
        assert!(inp.other_cards.iter().any(|c| c.starts_with("NLEG")));
        assert!(inp.other_cards.iter().any(|c| c.starts_with("RPATH")));
        assert!(inp.other_cards.iter().any(|c| c.starts_with("EXCHANGE")));

        // Comments should be stripped (not in other_cards)
        assert!(!inp.other_cards.iter().any(|c| c.contains("SCF")));
        assert!(!inp.other_cards.iter().any(|c| c.contains("POLARIZATION")));

        // Write and verify output
        let mut buf = Vec::new();
        inp.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();

        // EXCHANGE must be padded for ifx compatibility
        assert!(
            output.contains("EXCHANGE 0 0.0 0.0"),
            "EXCHANGE not padded: {output}"
        );

        // Must have END terminator
        assert!(output.trim_end().ends_with("END"));

        // Reparse and verify roundtrip fidelity
        let inp2 = FeffInput::parse(&output).unwrap();
        assert_eq!(inp2.edge, inp.edge);
        assert_eq!(inp2.s02, inp.s02);
        assert_eq!(inp2.control, inp.control);
        assert_eq!(inp2.print_flags, inp.print_flags);
        assert_eq!(inp2.potentials.len(), inp.potentials.len());
        assert_eq!(inp2.atoms.len(), inp.atoms.len());
        // EXCHANGE should now have 3 values in other_cards
        let exc = inp2
            .other_cards
            .iter()
            .find(|c| c.starts_with("EXCHANGE"))
            .unwrap();
        assert_eq!(exc, "EXCHANGE 0 0.0 0.0");
    }

    // --- validate() tests ---

    fn valid_input() -> FeffInput {
        FeffInput::parse(
            "\
TITLE test
EDGE K
S02 1.0
CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
POTENTIALS
0 29 Cu
1 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
1.805 1.805 0.0 1 Cu
END
",
        )
        .unwrap()
    }

    #[test]
    fn validate_valid_input() {
        valid_input().validate().unwrap();
    }

    #[test]
    fn validate_rejects_empty_potentials() {
        let mut inp = valid_input();
        inp.potentials.clear();
        let err = inp.validate().unwrap_err().to_string();
        assert!(err.contains("POTENTIALS section is empty"), "{err}");
    }

    #[test]
    fn validate_rejects_missing_absorber_potential() {
        let mut inp = valid_input();
        inp.potentials.retain(|p| p.ipot != 0);
        let err = inp.validate().unwrap_err().to_string();
        assert!(err.contains("ipot=0"), "{err}");
    }

    #[test]
    fn validate_rejects_empty_atoms() {
        let mut inp = valid_input();
        inp.atoms.clear();
        let err = inp.validate().unwrap_err().to_string();
        assert!(err.contains("ATOMS section is empty"), "{err}");
    }

    #[test]
    fn validate_rejects_undefined_ipot() {
        let mut inp = valid_input();
        inp.atoms.push(Atom {
            x: 2.0,
            y: 0.0,
            z: 0.0,
            ipot: 99,
            tag: "X".to_string(),
            distance: 0.0,
        });
        let err = inp.validate().unwrap_err().to_string();
        assert!(err.contains("undefined ipot=99"), "{err}");
    }

    #[test]
    fn validate_rejects_duplicate_ipot() {
        let mut inp = valid_input();
        inp.potentials.push(Potential {
            ipot: 0,
            z: 26,
            tag: "Fe".to_string(),
            l_scmt: None,
            l_fms: None,
            stoich: None,
        });
        let err = inp.validate().unwrap_err().to_string();
        assert!(err.contains("duplicate potential index ipot=0"), "{err}");
    }

    #[test]
    fn validate_rejects_invalid_z() {
        let mut inp = valid_input();
        inp.potentials[0].z = 0;
        let err = inp.validate().unwrap_err().to_string();
        assert!(err.contains("Z=0"), "{err}");

        inp.potentials[0].z = 200;
        let err = inp.validate().unwrap_err().to_string();
        assert!(err.contains("Z=200"), "{err}");
    }

    #[test]
    fn validate_rejects_no_absorber_atom() {
        let mut inp = valid_input();
        // Keep ipot=0 in potentials but remove all atoms that reference it
        inp.atoms.retain(|a| a.ipot != 0);
        let err = inp.validate().unwrap_err().to_string();
        assert!(err.contains("no atom with ipot=0"), "{err}");
    }

    #[test]
    fn validate_accepts_missing_edge() {
        let mut inp = valid_input();
        inp.edge = None;
        // EDGE is optional — FEFF defaults to K edge
        inp.validate().unwrap();
    }

    #[test]
    fn validate_reports_multiple_errors() {
        let inp = FeffInput::default(); // empty: no pots, no atoms
        let err = inp.validate().unwrap_err().to_string();
        assert!(err.contains("POTENTIALS"), "{err}");
        assert!(err.contains("ATOMS"), "{err}");
    }

    #[test]
    fn validate_fortran_rules_rejects_nohole_corehole_combo() {
        let content = "\
TITLE test
NOHOLE
COREHOLE RPA
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(err.contains("NOHOLE and COREHOLE"), "{err}");
    }

    #[test]
    fn validate_fortran_rules_rejects_atoms_and_overlap_combo() {
        let content = "\
TITLE test
OVERLAP 0
1 1 2.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(err.contains("ATOMS and OVERLAP"), "{err}");
    }

    #[test]
    fn validate_fortran_rules_rejects_mdff_without_nrixs() {
        let content = "\
TITLE test
XANES 8.0
MDFF 1
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(
            err.contains("MDFF option is only available with the NRIXS card"),
            "{err}"
        );
    }

    #[test]
    fn validate_fortran_rules_rejects_mdff2_without_two_q() {
        let content = "\
TITLE test
XANES 8.0
NRIXS 1 0.0 0.0 2.0
MDFF 2
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(err.contains("requires that you set nq=2"), "{err}");
    }

    #[test]
    fn validate_fortran_rules_rejects_hole_zero() {
        let content = "\
TITLE test
HOLE 0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let err = FeffInput::parse(content).unwrap_err().to_string();
        assert!(err.contains("Only ihole greater than zero"), "{err}");
    }

    #[test]
    fn write_to_preserves_card_order_for_parsed_input() {
        let content = "\
EXAFS 20.0
TITLE order
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let inp = FeffInput::parse(content).unwrap();
        let mut buf = Vec::new();
        inp.write_to(&mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.starts_with("EXAFS 20.0\nTITLE order\n"), "{out}");
    }

    #[test]
    fn write_canonical_keeps_normalized_layout() {
        let content = "\
TITLE canon
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let inp = FeffInput::parse(content).unwrap();
        let mut buf = Vec::new();
        inp.write_canonical(&mut buf).unwrap();
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("CONTROL 1 1 1 1 1 1"), "{out}");
        assert!(out.contains("PRINT 0 0 0 0 0 0"), "{out}");
        assert!(out.contains("POTENTIALS"), "{out}");
        assert!(out.contains("ATOMS"), "{out}");
    }

    #[test]
    fn normalize_exchange_pads_missing_fields() {
        assert_eq!(normalize_card("EXCHANGE 0"), "EXCHANGE 0 0.0 0.0");
        assert_eq!(normalize_card("EXCHANGE 0 0.5"), "EXCHANGE 0 0.5 0.0");
        assert_eq!(normalize_card("EXCHANGE 0 0.0 0.0"), "EXCHANGE 0 0.0 0.0");
        assert_eq!(
            normalize_card("EXCHANGE 0 0.0 0.0 2"),
            "EXCHANGE 0 0.0 0.0 2"
        );
    }

    #[test]
    fn normalize_preserves_other_cards() {
        assert_eq!(normalize_card("EXAFS 20.0"), "EXAFS 20.0");
        assert_eq!(normalize_card("RPATH 5.5"), "RPATH 5.5");
    }

    #[test]
    fn roundtrip_exchange_card() {
        let content = "\
TITLE test
EDGE K
S02 1.0
CONTROL 1 1 1 1 1 1
EXCHANGE 0
POTENTIALS
 0 29 Cu
 1 29 Cu
ATOMS
 0.00000 0.00000 0.00000 0 Cu 0.0
 1.80500 1.80500 0.00000 1 Cu 2.55270
END";
        let inp = FeffInput::parse(content).unwrap();
        let mut buf = Vec::new();
        inp.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("EXCHANGE 0 0.0 0.0"),
            "EXCHANGE should be padded: {output}"
        );
    }

    #[test]
    fn validate_bundled_examples() {
        if !has_submodule() {
            return;
        }
        let examples_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../feff10/examples");
        let dirs = ["EXAFS/Cu", "EXAFS/SF6", "XANES/Cu", "XANES/BN", "XES/Cu"];
        for d in dirs {
            let path = format!("{examples_dir}/{d}/feff.inp");
            let input = FeffInput::from_file(&path).unwrap();
            input.validate().unwrap_or_else(|e| {
                panic!("{d}/feff.inp failed validation: {e}");
            });
        }
    }
}
