use crate::support::common::pertab::atsym;

pub const MX_SAFE_N_POT: usize = 50;
pub const MX_RULE_NAME_LEN: usize = 8;
pub const POT_LABEL_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotGenRule {
    pub name: String,
}

impl Default for PotGenRule {
    fn default() -> Self {
        Self {
            name: "atomnum".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct XyzFormat {
    pub title: String,
    pub atomic_numbers: Vec<i32>,
    pub xyz: Vec<[f64; 3]>,
    pub potential_indices: Vec<usize>,
    pub potential_numeric_labels: Vec<i32>,
    pub potential_string_labels: Vec<String>,
}

impl XyzFormat {
    pub fn atom_count(&self) -> usize {
        self.atomic_numbers.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotListType {
    pub atomic_numbers: Vec<i32>,
    pub numeric_labels: Vec<i32>,
    pub string_labels: Vec<String>,
}

impl PotListType {
    pub fn n_pot(&self) -> usize {
        self.atomic_numbers.len().saturating_sub(1)
    }
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum PotGeneratorError {
    #[error("structure must include at least one atom")]
    EmptyStructure,
    #[error(
        "atomic number and coordinate lengths must match (atomic_numbers={atomic_numbers}, xyz={xyz})"
    )]
    InvalidStructureShape { atomic_numbers: usize, xyz: usize },
    #[error("absorber index is out of range (absorber={absorber}, nat={nat})")]
    AbsorberOutOfRange { absorber: usize, nat: usize },
    #[error("potential assignment incomplete at atom index {0}")]
    IncompleteAssignment(usize),
}

pub fn gen_pot_from_xyz(
    rule: &PotGenRule,
    absorber_index_1based: usize,
    structure: &mut XyzFormat,
) -> Result<PotListType, PotGeneratorError> {
    validate_structure(absorber_index_1based, structure)?;

    let rule_name = rule.name.trim().to_ascii_lowercase();
    let pot_list = match rule_name.as_str() {
        "atomnum" => rule_atomnum(absorber_index_1based, structure)?,
        _ => rule_atomnum(absorber_index_1based, structure)?,
    };

    structure.potential_numeric_labels = vec![0; structure.atom_count()];
    structure.potential_string_labels = vec![String::new(); structure.atom_count()];
    for atom_index in 0..structure.atom_count() {
        let potential_index = structure.potential_indices[atom_index];
        structure.potential_numeric_labels[atom_index] = pot_list.numeric_labels[potential_index];
        structure.potential_string_labels[atom_index] =
            pot_list.string_labels[potential_index].clone();
    }

    Ok(pot_list)
}

fn validate_structure(
    absorber_index_1based: usize,
    structure: &XyzFormat,
) -> Result<(), PotGeneratorError> {
    let nat = structure.atom_count();
    if nat == 0 {
        return Err(PotGeneratorError::EmptyStructure);
    }
    if structure.xyz.len() != nat {
        return Err(PotGeneratorError::InvalidStructureShape {
            atomic_numbers: nat,
            xyz: structure.xyz.len(),
        });
    }
    if !(1..=nat).contains(&absorber_index_1based) {
        return Err(PotGeneratorError::AbsorberOutOfRange {
            absorber: absorber_index_1based,
            nat,
        });
    }
    Ok(())
}

fn rule_atomnum(
    absorber_index_1based: usize,
    structure: &mut XyzFormat,
) -> Result<PotListType, PotGeneratorError> {
    let nat = structure.atom_count();
    let absorber_index = absorber_index_1based - 1;

    structure.potential_indices = vec![usize::MAX; nat];
    structure.potential_indices[absorber_index] = 0;

    let absorber_atomic_number = structure.atomic_numbers[absorber_index];
    let mut unique_atomic_numbers = structure.atomic_numbers.clone();
    unique_atomic_numbers.sort_unstable();
    unique_atomic_numbers.dedup();

    let absorber_count = structure
        .atomic_numbers
        .iter()
        .filter(|&&atomic_number| atomic_number == absorber_atomic_number)
        .count();
    let n_pot = if absorber_count > 1 {
        unique_atomic_numbers.len()
    } else {
        unique_atomic_numbers.len().saturating_sub(1)
    };

    let mut pot_list = PotListType {
        atomic_numbers: vec![0; n_pot + 1],
        numeric_labels: vec![0; n_pot + 1],
        string_labels: vec![String::new(); n_pot + 1],
    };

    pot_list.atomic_numbers[0] = absorber_atomic_number;
    pot_list.numeric_labels[0] = 0;
    pot_list.string_labels[0] = format!("{}_Abs", atomic_symbol(absorber_atomic_number));

    let mut potential_index = 0_usize;
    for unique_atomic_number in unique_atomic_numbers {
        let has_assignable_atoms = structure
            .atomic_numbers
            .iter()
            .zip(structure.potential_indices.iter())
            .any(|(&atomic_number, &assigned)| {
                atomic_number == unique_atomic_number && assigned != 0
            });
        if !has_assignable_atoms {
            continue;
        }

        potential_index += 1;
        for atom_index in 0..nat {
            if structure.atomic_numbers[atom_index] == unique_atomic_number
                && structure.potential_indices[atom_index] != 0
            {
                structure.potential_indices[atom_index] = potential_index;
            }
        }

        pot_list.atomic_numbers[potential_index] = unique_atomic_number;
        pot_list.numeric_labels[potential_index] = potential_index as i32;
        pot_list.string_labels[potential_index] = atomic_symbol(unique_atomic_number).to_string();
    }

    if let Some((index, _)) = structure
        .potential_indices
        .iter()
        .enumerate()
        .find(|(_, assigned)| **assigned == usize::MAX)
    {
        return Err(PotGeneratorError::IncompleteAssignment(index + 1));
    }

    Ok(pot_list)
}

fn atomic_symbol(atomic_number: i32) -> &'static str {
    if atomic_number <= 0 {
        return "X";
    }
    atsym(atomic_number as usize).unwrap_or("X")
}

#[cfg(test)]
mod tests {
    use super::{PotGenRule, PotGeneratorError, XyzFormat, gen_pot_from_xyz};

    #[test]
    fn atomnum_rule_assigns_absorber_and_species_potentials() {
        let mut structure = XyzFormat {
            title: "CuO".to_string(),
            atomic_numbers: vec![29, 29, 8, 8],
            xyz: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 1.0],
            ],
            potential_indices: Vec::new(),
            potential_numeric_labels: Vec::new(),
            potential_string_labels: Vec::new(),
        };

        let pot_list = gen_pot_from_xyz(&PotGenRule::default(), 1, &mut structure)
            .expect("atomnum mapping should succeed");

        assert_eq!(pot_list.atomic_numbers, vec![29, 8, 29]);
        assert_eq!(structure.potential_indices, vec![0, 2, 1, 1]);
        assert_eq!(structure.potential_numeric_labels, vec![0, 2, 1, 1]);
        assert_eq!(structure.potential_string_labels[0], "Cu_Abs");
    }

    #[test]
    fn atomnum_rule_handles_unique_absorber_species() {
        let mut structure = XyzFormat {
            title: "CuO2".to_string(),
            atomic_numbers: vec![29, 8, 8],
            xyz: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            potential_indices: Vec::new(),
            potential_numeric_labels: Vec::new(),
            potential_string_labels: Vec::new(),
        };

        let pot_list = gen_pot_from_xyz(&PotGenRule::default(), 1, &mut structure)
            .expect("atomnum mapping should succeed");

        assert_eq!(pot_list.n_pot(), 1);
        assert_eq!(structure.potential_indices, vec![0, 1, 1]);
    }

    #[test]
    fn invalid_absorber_index_is_rejected() {
        let mut structure = XyzFormat {
            title: "X".to_string(),
            atomic_numbers: vec![8],
            xyz: vec![[0.0, 0.0, 0.0]],
            potential_indices: Vec::new(),
            potential_numeric_labels: Vec::new(),
            potential_string_labels: Vec::new(),
        };

        let error = gen_pot_from_xyz(&PotGenRule::default(), 2, &mut structure)
            .expect_err("invalid absorber should fail");
        assert_eq!(
            error,
            PotGeneratorError::AbsorberOutOfRange {
                absorber: 2,
                nat: 1
            }
        );
    }
}
