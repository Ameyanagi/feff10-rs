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

            let upper = line.to_uppercase();

            // Check for section terminators
            if upper.starts_with("END") {
                in_potentials = false;
                in_atoms = false;
                continue;
            }

            // If we're in a section, parse data lines
            if in_potentials {
                if upper.starts_with(|c: char| c.is_alphabetic()) {
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
                if upper.starts_with(|c: char| c.is_alphabetic()) {
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

            // Parse card keywords
            if upper.starts_with("TITLE") {
                let rest = line.get(5..).unwrap_or("").trim();
                input.title.push(rest.to_string());
            } else if upper.starts_with("EDGE") {
                let rest = line.get(4..).unwrap_or("").trim();
                input.edge = Some(rest.to_string());
            } else if upper.starts_with("S02") {
                let rest = line.get(3..).unwrap_or("").trim();
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
            } else if upper.starts_with("CONTROL") {
                let rest = line.get(7..).unwrap_or("").trim();
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
            } else if upper.starts_with("PRINT") {
                let rest = line.get(5..).unwrap_or("").trim();
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
            } else if upper.starts_with("POTENTIALS") {
                in_potentials = true;
            } else if upper.starts_with("ATOMS") {
                in_atoms = true;
            } else {
                // Preserve other cards verbatim
                input.other_cards.push(line.to_string());
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
    if parts.len() < 5 {
        return None;
    }
    let x = parts[0].parse().ok()?;
    let y = parts[1].parse().ok()?;
    let z = parts[2].parse().ok()?;
    let ipot = parts[3].parse().ok()?;
    let tag = parts[4].to_string();
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
    if parts.len() < 5 {
        return Err(parse_error(
            line_num,
            "ATOMS line must contain at least: x y z ipot tag",
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
    let tag = parts[4].to_string();
    let distance = parts
        .get(5)
        .map(|s| {
            s.parse::<f64>()
                .map_err(|_| parse_error(line_num, format!("invalid atom distance '{}'", s)))
        })
        .transpose()?
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

    #[test]
    fn parse_exafs_cu() {
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
}
