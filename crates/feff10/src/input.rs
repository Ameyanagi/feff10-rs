use std::collections::HashSet;
use std::io::Write;
use std::path::Path;

use crate::error::{Error, ParseError};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParseMode {
    Permissive,
    Strict,
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
        Self::parse_with_mode(content, ParseMode::Permissive)
    }

    /// Parse a feff.inp file from a string with strict validation.
    pub fn parse_strict(content: &str) -> Result<Self, Error> {
        Self::parse_with_mode(content, ParseMode::Strict)
    }

    fn parse_with_mode(content: &str, mode: ParseMode) -> Result<Self, Error> {
        let mut input = FeffInput::default();
        let mut in_potentials = false;
        let mut in_atoms = false;

        for (line_idx, line) in content.lines().enumerate() {
            let line_num = line_idx + 1;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() {
                continue;
            }
            if line.starts_with('*') {
                continue;
            }

            // Extract the first word (keyword) for matching
            let first_word = line.split_whitespace().next().unwrap_or("");
            let keyword = first_word.to_uppercase();
            let rest = line[first_word.len()..].trim();

            // Check for section terminators
            if keyword == "END" {
                in_potentials = false;
                in_atoms = false;
                continue;
            }

            // If we're in a section, parse data lines
            if in_potentials {
                if first_word.starts_with(|c: char| c.is_alphabetic()) {
                    // New card keyword encountered - exit potentials section
                    in_potentials = false;
                } else {
                    if mode == ParseMode::Strict {
                        let pot = parse_potential_line_strict(line, line_num)?;
                        input.potentials.push(pot);
                    } else if let Some(pot) = parse_potential_line(line) {
                        input.potentials.push(pot);
                        continue;
                    }
                    continue;
                }
            }

            if in_atoms {
                if first_word.starts_with(|c: char| c.is_alphabetic()) {
                    in_atoms = false;
                } else {
                    if mode == ParseMode::Strict {
                        let atom = parse_atom_line_strict(line, line_num)?;
                        input.atoms.push(atom);
                    } else if let Some(atom) = parse_atom_line(line) {
                        input.atoms.push(atom);
                        continue;
                    }
                    continue;
                }
            }

            // Parse card keywords (exact first-word match to avoid prefix collisions)
            match keyword.as_str() {
                "TITLE" => {
                    input.title.push(rest.to_string());
                }
                "EDGE" => {
                    input.edge = Some(rest.to_string());
                }
                "S02" => {
                    if rest.is_empty() {
                        if mode == ParseMode::Strict {
                            return Err(parse_error(line_num, "S02 requires a numeric value"));
                        }
                    } else {
                        match rest.parse::<f64>() {
                            Ok(val) => input.s02 = Some(val),
                            Err(_) if mode == ParseMode::Strict => {
                                return Err(parse_error(
                                    line_num,
                                    format!("invalid S02 value '{rest}'"),
                                ));
                            }
                            Err(_) => {}
                        }
                    }
                }
                "CONTROL" => {
                    if mode == ParseMode::Strict {
                        input.control = parse_six_u32(rest, line_num, "CONTROL")?;
                    } else {
                        let vals: Vec<u32> = rest
                            .split_whitespace()
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        for (i, &v) in vals.iter().enumerate().take(6) {
                            input.control[i] = v;
                        }
                    }
                }
                "PRINT" => {
                    if mode == ParseMode::Strict {
                        input.print_flags = parse_six_u32(rest, line_num, "PRINT")?;
                    } else {
                        let vals: Vec<u32> = rest
                            .split_whitespace()
                            .filter_map(|s| s.parse().ok())
                            .collect();
                        for (i, &v) in vals.iter().enumerate().take(6) {
                            input.print_flags[i] = v;
                        }
                    }
                }
                "POTENTIALS" => {
                    in_potentials = true;
                }
                "ATOMS" => {
                    in_atoms = true;
                }
                _ => {
                    // Preserve other cards verbatim
                    input.other_cards.push(line.to_string());
                }
            }
        }

        Ok(input)
    }

    /// Parse from a file path.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse from a file path with strict validation.
    pub fn from_file_strict(path: impl AsRef<Path>) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)?;
        Self::parse_strict(&content)
    }

    /// Write feff.inp to a writer.
    pub fn write_to(&self, w: &mut dyn Write) -> std::io::Result<()> {
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

        // Write other cards
        writeln!(w)?;
        for card in &self.other_cards {
            writeln!(w, "{card}")?;
        }

        // Potentials
        writeln!(w)?;
        writeln!(w, "POTENTIALS")?;
        for pot in &self.potentials {
            write!(w, "{:>5} {:>3} {:<6}", pot.ipot, pot.z, pot.tag)?;
            if let Some(l) = pot.l_scmt {
                write!(w, " {l}")?;
            }
            if let Some(l) = pot.l_fms {
                write!(w, " {l}")?;
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

        if errors.is_empty() {
            Ok(())
        } else {
            Err(Error::Config(errors.join("\n")))
        }
    }
}

fn parse_potential_line(line: &str) -> Option<Potential> {
    // Strip comments
    let line = strip_inline_comment(line);
    if line.is_empty() {
        return None;
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let ipot = parts[0].parse().ok()?;
    let z = parts[1].parse().ok()?;
    let tag = parts[2].to_string();
    let l_scmt = parts.get(3).and_then(|s| s.parse().ok());
    let l_fms = parts.get(4).and_then(|s| s.parse().ok());
    let stoich = parts.get(5).and_then(|s| s.parse().ok());
    Some(Potential {
        ipot,
        z,
        tag,
        l_scmt,
        l_fms,
        stoich,
    })
}

fn parse_atom_line(line: &str) -> Option<Atom> {
    let line = strip_inline_comment(line);
    if line.is_empty() {
        return None;
    }
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }
    let x = parts[0].parse().ok()?;
    let y = parts[1].parse().ok()?;
    let z = parts[2].parse().ok()?;
    let ipot = parts[3].parse().ok()?;
    // tag is optional; if the 5th field is non-numeric text, use it as tag
    let tag = parts.get(4).map(|s| s.to_string()).unwrap_or_default();
    // distance is optional; try parsing from whichever field follows the tag
    let distance = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0.0);
    Some(Atom {
        x,
        y,
        z,
        ipot,
        tag,
        distance,
    })
}

fn parse_error(line: usize, message: impl Into<String>) -> Error {
    Error::Parse(ParseError {
        line,
        message: message.into(),
    })
}

fn parse_six_u32(rest: &str, line: usize, card: &str) -> Result<[u32; 6], Error> {
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    if tokens.len() != 6 {
        return Err(parse_error(
            line,
            format!("{card} must have exactly 6 integer values"),
        ));
    }
    let mut out = [0_u32; 6];
    for (i, token) in tokens.iter().enumerate() {
        out[i] = token.parse::<u32>().map_err(|_| {
            parse_error(
                line,
                format!("{card} value {} is not a valid integer: '{token}'", i + 1),
            )
        })?;
    }
    Ok(out)
}

fn parse_potential_line_strict(line: &str, line_num: usize) -> Result<Potential, Error> {
    let line = strip_inline_comment(line);
    if line.is_empty() {
        return Err(parse_error(line_num, "empty line in POTENTIALS section"));
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 3 {
        return Err(parse_error(
            line_num,
            "POTENTIALS line must contain at least: ipot z tag",
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
    let tag = parts[2].to_string();
    let l_scmt = parts
        .get(3)
        .map(|s| {
            s.parse::<u32>()
                .map_err(|_| parse_error(line_num, format!("invalid POTENTIALS l_scmt '{}'", s)))
        })
        .transpose()?;
    let l_fms = parts
        .get(4)
        .map(|s| {
            s.parse::<u32>()
                .map_err(|_| parse_error(line_num, format!("invalid POTENTIALS l_fms '{}'", s)))
        })
        .transpose()?;
    let stoich = parts
        .get(5)
        .map(|s| {
            s.parse::<f64>()
                .map_err(|_| parse_error(line_num, format!("invalid POTENTIALS stoich '{}'", s)))
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
    let line = strip_inline_comment(line);
    if line.is_empty() {
        return Err(parse_error(line_num, "empty line in ATOMS section"));
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return Err(parse_error(
            line_num,
            "ATOMS line must contain at least: x y z ipot",
        ));
    }

    let x = parts[0].parse::<f64>().map_err(|_| {
        parse_error(
            line_num,
            format!("invalid atom x coordinate '{}'", parts[0]),
        )
    })?;
    let y = parts[1].parse::<f64>().map_err(|_| {
        parse_error(
            line_num,
            format!("invalid atom y coordinate '{}'", parts[1]),
        )
    })?;
    let z = parts[2].parse::<f64>().map_err(|_| {
        parse_error(
            line_num,
            format!("invalid atom z coordinate '{}'", parts[2]),
        )
    })?;
    let ipot = parts[3]
        .parse::<u32>()
        .map_err(|_| parse_error(line_num, format!("invalid atom ipot '{}'", parts[3])))?;
    // tag is optional (FEFF only requires x y z ipot)
    let tag = parts.get(4).map(|s| s.to_string()).unwrap_or_default();
    // distance is optional; non-numeric trailing text is ignored as a label/comment
    let distance = parts
        .get(5)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    Ok(Atom {
        x,
        y,
        z,
        ipot,
        tag,
        distance,
    })
}

fn strip_inline_comment(line: &str) -> &str {
    line.split('*').next().unwrap_or("").trim()
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
    fn parse_inline_comment_in_potentials() {
        let content = "\
POTENTIALS
0 29 Cu * absorber
1 26 Fe * scatterer
ATOMS
0.0 0.0 0.0 0 Cu
END
";
        let input = FeffInput::parse(content).unwrap();
        assert_eq!(input.potentials.len(), 2);
        assert_eq!(input.potentials[0].tag, "Cu");
        assert_eq!(input.potentials[1].tag, "Fe");
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
