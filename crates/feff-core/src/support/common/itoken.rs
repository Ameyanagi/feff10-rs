#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputDeckKind {
    Feff,
    Spring,
}

const FEFF_PREFIX_TO_TOKEN: [(&str, i32); 116] = [
    ("ATOM", 1),
    ("HOLE", 2),
    ("OVER", 3),
    ("CONT", 4),
    ("EXCH", 5),
    ("ION ", 6),
    ("TITL", 7),
    ("FOLP", 8),
    ("RPAT", 9),
    ("RMAX", 9),
    ("DEBY", 10),
    ("RMUL", 11),
    ("SS  ", 12),
    ("PRIN", 13),
    ("POTE", 14),
    ("NLEG", 15),
    ("CRIT", 16),
    ("NOGE", 17),
    ("IORD", 18),
    ("PCRI", 19),
    ("SIG2", 20),
    ("XANE", 21),
    ("CORR", 22),
    ("AFOL", 23),
    ("EXAF", 24),
    ("POLA", 25),
    ("ELLI", 26),
    ("RGRI", 27),
    ("RPHA", 28),
    ("NSTA", 29),
    ("NOHO", 30),
    ("SIG3", 31),
    ("JUMP", 32),
    ("MBCO", 33),
    ("SPIN", 34),
    ("EDGE", 35),
    ("SCF ", 36),
    ("FMS ", 37),
    ("LDOS", 38),
    ("INTE", 39),
    ("CFAV", 40),
    ("S02 ", 41),
    ("XES ", 42),
    ("DANE", 43),
    ("FPRI", 44),
    ("RSIG", 45),
    ("XNCD", 46),
    ("XMCD", 46),
    ("MULT", 47),
    ("UNFR", 48),
    ("TDLD", 49),
    ("PMBS", 50),
    ("PLAS", 51),
    ("MPSE", 51),
    ("SO2C", 52),
    ("SFCO", 52),
    ("SELF", 53),
    ("SFSE", 54),
    ("RCON", 55),
    ("ELNE", 56),
    ("EXEL", 57),
    ("MAGI", 58),
    ("ABSO", 59),
    ("SYMM", 60),
    ("REAL", 61),
    ("RECI", 62),
    ("SGRO", 63),
    ("LATT", 64),
    ("KMES", 65),
    ("STRF", 66),
    ("BAND", 67),
    ("CORE", 68),
    ("MARK", 71),
    ("TARG", 71),
    ("EGRI", 72),
    ("COOR", 73),
    ("EXTP", 74),
    ("CHBR", 75),
    ("CHSH", 76),
    ("DIMS", 77),
    ("NRIX", 78),
    ("LJMA", 79),
    ("LDEC", 80),
    ("SETE", 81),
    ("EPS0", 82),
    ("OPCO", 83),
    ("NUMD", 84),
    ("PREP", 85),
    ("EGAP", 86),
    ("CHWI", 87),
    ("MDFF", 88),
    ("REST", 89),
    ("CONF", 90),
    ("SCRE", 91),
    ("CIF ", 92),
    ("EQUI", 93),
    ("COMP", 94),
    ("RHOZ", 95),
    ("CGRI", 96),
    ("CORV", 97),
    ("SIGG", 98),
    ("TEMP", 99),
    ("DENS", 100),
    ("RIXS", 101),
    ("RLPR", 102),
    ("ICOR", 103),
    ("HUBB", 104),
    ("CRPA", 105),
    ("FULL", 106),
    ("SCXC", 107),
    ("HIGH", 108),
    ("SCFT", 109),
    ("WARN", 110),
    ("SCFR", 111),
    ("TOLS", 112),
    ("END ", -1),
];

const FEFF_TOKEN_TO_KEYWORD: [(&str, i32); 110] = [
    ("ATOMS", 1),
    ("HOLE", 2),
    ("OVERLAP", 3),
    ("CONTROL", 4),
    ("EXCHANGE", 5),
    ("ION", 6),
    ("TITLE", 7),
    ("FOLP", 8),
    ("RPATH", 9),
    ("DEBYE", 10),
    ("RMULT", 11),
    ("SS", 12),
    ("PRINT", 13),
    ("POTENTIALS", 14),
    ("NLEG", 15),
    ("CRITERIA", 16),
    ("NOGEOM", 17),
    ("IORD", 18),
    ("PCRITERIA", 19),
    ("SIG2", 20),
    ("XANES", 21),
    ("CORRECTIONS", 22),
    ("AFOLP", 23),
    ("EXAFS", 24),
    ("POLARIZATION", 25),
    ("ELLIPTICITY", 26),
    ("RGRID", 27),
    ("RPHASES", 28),
    ("NSTAR", 29),
    ("NOHOLE", 30),
    ("SIG3", 31),
    ("JUMPRM", 32),
    ("MBCONV", 33),
    ("SPIN", 34),
    ("EDGE", 35),
    ("SCF", 36),
    ("FMS", 37),
    ("LDOS", 38),
    ("INTERSTITIAL", 39),
    ("CFAVERAGE", 40),
    ("S02", 41),
    ("XES", 42),
    ("DANES", 43),
    ("FPRIME", 44),
    ("RSIGMA", 45),
    ("XMCD", 46),
    ("MULT", 47),
    ("UNFREEZEF", 48),
    ("TDLDA", 49),
    ("PMBSE", 50),
    ("MPSE", 51),
    ("SFCONV", 52),
    ("SELF", 53),
    ("SFSE", 54),
    ("RCONV", 55),
    ("ELNES", 56),
    ("EXELFS", 57),
    ("MAGIC", 58),
    ("ABSOLUTE", 59),
    ("SYMMETRY", 60),
    ("REAL", 61),
    ("RECIPROCAL", 62),
    ("SGROUP", 63),
    ("LATTICE", 64),
    ("KMESH", 65),
    ("STRFAC", 66),
    ("BAND", 67),
    ("COREHOLE", 68),
    ("TARGET", 71),
    ("EGRID", 72),
    ("COORDINATES", 73),
    ("EXTPOT", 74),
    ("CHBROADENING", 75),
    ("CHSHIFT", 76),
    ("DIMS", 77),
    ("NRIXS", 78),
    ("LJMAX", 79),
    ("LDECMX", 80),
    ("SETE", 81),
    ("EPS0", 82),
    ("OPCONS", 83),
    ("NUMD", 84),
    ("PREP", 85),
    ("EGAP", 86),
    ("CHWIDTH", 87),
    ("MDFF", 88),
    ("RESTART", 89),
    ("CONFIGURATION", 90),
    ("SCREEN", 91),
    ("CIF", 92),
    ("EQUIVALENCE", 93),
    ("COMPTON", 94),
    ("RHOZZP", 95),
    ("CGRID", 96),
    ("CORVAL", 97),
    ("SIGGK", 98),
    ("TEMP", 99),
    ("DENS", 100),
    ("RIXS", 101),
    ("RLPR", 102),
    ("ICOR", 103),
    ("HUBBARD", 104),
    ("CRPA", 105),
    ("FULLSPECTRUM", 106),
    ("SCXC", 107),
    ("HIGHZ", 108),
    ("SCFTH", 109),
    ("WARN", 110),
    ("SCFR", 111),
    ("TOLS", 112),
];

const SPRING_PREFIX_TO_TOKEN: [(&str, i32); 6] = [
    ("STRE", 1),
    ("ANGL", 2),
    ("VDOS", 3),
    ("PRDO", 4),
    ("PRIN", 4),
    ("END ", -1),
];

const SPRING_TOKEN_TO_KEYWORD: [(&str, i32); 4] =
    [("STRETCH", 1), ("ANGLE", 2), ("VDOS", 3), ("PRDOS", 4)];

pub fn itoken(word: &str, file_name: &str) -> i32 {
    let Some(kind) = input_deck_kind(file_name) else {
        return 0;
    };
    let prefix = token_prefix(word);
    match kind {
        InputDeckKind::Feff => FEFF_PREFIX_TO_TOKEN
            .iter()
            .find_map(|(candidate, token)| (*candidate == prefix).then_some(*token))
            .unwrap_or(0),
        InputDeckKind::Spring => SPRING_PREFIX_TO_TOKEN
            .iter()
            .find_map(|(candidate, token)| (*candidate == prefix).then_some(*token))
            .unwrap_or(0),
    }
}

pub fn itoken_reverse(file_name: &str, token: i32) -> Option<&'static str> {
    let kind = input_deck_kind(file_name)?;
    if token == -1 {
        return Some("END");
    }

    match kind {
        InputDeckKind::Feff => FEFF_TOKEN_TO_KEYWORD
            .iter()
            .find_map(|(keyword, mapped)| (*mapped == token).then_some(*keyword)),
        InputDeckKind::Spring => SPRING_TOKEN_TO_KEYWORD
            .iter()
            .find_map(|(keyword, mapped)| (*mapped == token).then_some(*keyword)),
    }
}

pub fn canonical_keyword(word: &str, file_name: &str) -> Option<&'static str> {
    let token = itoken(word, file_name);
    if token == 0 {
        return None;
    }
    itoken_reverse(file_name, token)
}

pub fn canonical_keyword_for_parser(keyword: &str) -> Option<&'static str> {
    if let Some(canonical) = canonical_keyword(keyword, "feff.inp") {
        return Some(canonical);
    }

    let spring = canonical_keyword(keyword, "spring.inp")?;
    if spring == "STRETCH" {
        Some("STRETCHES")
    } else {
        Some(spring)
    }
}

fn input_deck_kind(file_name: &str) -> Option<InputDeckKind> {
    let normalized = file_name.trim().to_ascii_lowercase();
    if normalized.ends_with("feff.inp") {
        Some(InputDeckKind::Feff)
    } else if normalized.ends_with("spring.inp") {
        Some(InputDeckKind::Spring)
    } else {
        None
    }
}

fn token_prefix(word: &str) -> String {
    let mut prefix = [' '; 4];
    for (index, character) in word.trim().chars().take(4).enumerate() {
        prefix[index] = character.to_ascii_uppercase();
    }
    prefix.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::{canonical_keyword_for_parser, itoken, itoken_reverse};

    #[test]
    fn itoken_matches_feff_prefix_rules() {
        assert_eq!(itoken("POTENTIALS", "feff.inp"), 14);
        assert_eq!(itoken("RMAX", "feff.inp"), 9);
        assert_eq!(itoken("end", "feff.inp"), -1);
    }

    #[test]
    fn itoken_reverse_matches_feff_keyword_mapping() {
        assert_eq!(itoken_reverse("feff.inp", 14), Some("POTENTIALS"));
        assert_eq!(itoken_reverse("feff.inp", 52), Some("SFCONV"));
        assert_eq!(itoken_reverse("feff.inp", -1), Some("END"));
    }

    #[test]
    fn canonical_keyword_for_parser_maps_spring_stretch_prefix() {
        assert_eq!(canonical_keyword_for_parser("STRE"), Some("STRETCHES"));
        assert_eq!(canonical_keyword_for_parser("VDOS"), Some("VDOS"));
    }

    #[test]
    fn canonical_keyword_for_parser_maps_feff_prefixes() {
        assert_eq!(canonical_keyword_for_parser("TITL"), Some("TITLE"));
        assert_eq!(canonical_keyword_for_parser("POTE"), Some("POTENTIALS"));
    }

    #[test]
    fn canonical_keyword_for_parser_returns_none_for_unknown_keyword() {
        assert_eq!(canonical_keyword_for_parser("FUTU"), None);
    }
}
