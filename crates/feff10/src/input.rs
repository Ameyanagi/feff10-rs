use std::io::Write;
use std::path::Path;

use crate::error::Error;

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
        let mut input = FeffInput::default();
        let mut in_potentials = false;
        let mut in_atoms = false;

        for (_line_num, line) in content.lines().enumerate() {
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
                if upper.starts_with(|c: char| c.is_alphabetic()) && !upper.starts_with(|c: char| c.is_ascii_digit()) {
                    // New card keyword encountered - exit potentials section
                    in_potentials = false;
                } else {
                    if let Some(pot) = parse_potential_line(line) {
                        input.potentials.push(pot);
                        continue;
                    }
                    continue;
                }
            }

            if in_atoms {
                if upper.starts_with(|c: char| c.is_alphabetic()) && !upper.starts_with('-') {
                    in_atoms = false;
                } else {
                    if let Some(atom) = parse_atom_line(line) {
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
                if let Ok(val) = rest.parse::<f64>() {
                    input.s02 = Some(val);
                }
            } else if upper.starts_with("CONTROL") {
                let rest = line.get(7..).unwrap_or("").trim();
                let vals: Vec<u32> = rest
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                for (i, &v) in vals.iter().enumerate().take(6) {
                    input.control[i] = v;
                }
            } else if upper.starts_with("PRINT") {
                let rest = line.get(5..).unwrap_or("").trim();
                let vals: Vec<u32> = rest
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                for (i, &v) in vals.iter().enumerate().take(6) {
                    input.print_flags[i] = v;
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
    let line = line.split('*').next()?.trim();
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
    let line = line.split('*').next()?.trim();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_exafs_cu() {
        let content = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../feff10/examples/EXAFS/Cu/feff.inp")
        ).unwrap();
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
        let content = std::fs::read_to_string(
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../feff10/examples/EXAFS/Cu/feff.inp")
        ).unwrap();
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
}
