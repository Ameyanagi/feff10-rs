use super::m_pot_generator::{
    PotGenRule, PotGeneratorError, PotListType, XyzFormat, gen_pot_from_xyz,
};
use crate::support::common::pertab::atsym;

#[derive(Debug, Clone, PartialEq)]
pub struct PotGeneratorTestInput {
    pub rule: PotGenRule,
    pub absorber_index: usize,
    pub structure: XyzFormat,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum PotGeneratorTestError {
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("invalid integer for {field}: {value}")]
    InvalidInteger { field: &'static str, value: String },
    #[error("invalid float for {field}: {value}")]
    InvalidFloat { field: &'static str, value: String },
    #[error("unknown atomic symbol '{0}'")]
    UnknownAtomicSymbol(String),
    #[error(transparent)]
    PotGenerator(#[from] PotGeneratorError),
}

pub fn parse_pot_generator_test_input(
    source: &str,
) -> Result<PotGeneratorTestInput, PotGeneratorTestError> {
    let mut lines = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .into_iter();

    let rule_name = lines
        .next()
        .ok_or(PotGeneratorTestError::MissingField("rule"))?;
    let absorber_line = lines
        .next()
        .ok_or(PotGeneratorTestError::MissingField("absorber_index"))?;
    let atom_count_line = lines
        .next()
        .ok_or(PotGeneratorTestError::MissingField("nAt"))?;
    let title = lines
        .next()
        .ok_or(PotGeneratorTestError::MissingField("title"))?;

    let absorber_index = parse_usize(absorber_line, "absorber_index")?;
    let atom_count = parse_usize(atom_count_line, "nAt")?;

    let mut atomic_numbers = Vec::with_capacity(atom_count);
    let mut xyz = Vec::with_capacity(atom_count);
    for _ in 0..atom_count {
        let atom_line = lines
            .next()
            .ok_or(PotGeneratorTestError::MissingField("atom row"))?;
        let columns = atom_line.split_whitespace().collect::<Vec<_>>();
        if columns.len() < 4 {
            return Err(PotGeneratorTestError::MissingField(
                "atom row requires symbol x y z",
            ));
        }

        let atomic_number = atomic_number_for_symbol(columns[0])?;
        let x = parse_f64(columns[1], "x")?;
        let y = parse_f64(columns[2], "y")?;
        let z = parse_f64(columns[3], "z")?;

        atomic_numbers.push(atomic_number);
        xyz.push([x, y, z]);
    }

    Ok(PotGeneratorTestInput {
        rule: PotGenRule {
            name: rule_name.to_string(),
        },
        absorber_index,
        structure: XyzFormat {
            title: title.to_string(),
            atomic_numbers,
            xyz,
            potential_indices: Vec::new(),
            potential_numeric_labels: Vec::new(),
            potential_string_labels: Vec::new(),
        },
    })
}

pub fn run_pot_generator_test(
    source: &str,
) -> Result<(PotListType, XyzFormat), PotGeneratorTestError> {
    let mut parsed = parse_pot_generator_test_input(source)?;
    let pot_list = gen_pot_from_xyz(&parsed.rule, parsed.absorber_index, &mut parsed.structure)?;
    Ok((pot_list, parsed.structure))
}

fn parse_usize(value: &str, field: &'static str) -> Result<usize, PotGeneratorTestError> {
    value
        .parse::<usize>()
        .map_err(|_| PotGeneratorTestError::InvalidInteger {
            field,
            value: value.to_string(),
        })
}

fn parse_f64(value: &str, field: &'static str) -> Result<f64, PotGeneratorTestError> {
    value
        .replace(['D', 'd'], "E")
        .parse::<f64>()
        .map_err(|_| PotGeneratorTestError::InvalidFloat {
            field,
            value: value.to_string(),
        })
}

fn atomic_number_for_symbol(symbol: &str) -> Result<i32, PotGeneratorTestError> {
    for atomic_number in 1..=139 {
        if let Some(existing_symbol) = atsym(atomic_number)
            && existing_symbol.eq_ignore_ascii_case(symbol)
        {
            return Ok(atomic_number as i32);
        }
    }
    Err(PotGeneratorTestError::UnknownAtomicSymbol(
        symbol.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{PotGeneratorTestError, parse_pot_generator_test_input, run_pot_generator_test};

    const INPUT: &str = "\
atomnum
1
4
CuO structure
Cu 0.0 0.0 0.0
Cu 1.0 0.0 0.0
O 0.0 1.0 0.0
O 0.0 0.0 1.0
";

    #[test]
    fn parser_reads_legacy_test_program_input() {
        let parsed = parse_pot_generator_test_input(INPUT).expect("input should parse");
        assert_eq!(parsed.absorber_index, 1);
        assert_eq!(parsed.structure.atomic_numbers, vec![29, 29, 8, 8]);
        assert_eq!(parsed.structure.xyz.len(), 4);
    }

    #[test]
    fn runner_executes_atomnum_mapping() {
        let (pot_list, structure) = run_pot_generator_test(INPUT).expect("runner should succeed");
        assert_eq!(pot_list.atomic_numbers, vec![29, 8, 29]);
        assert_eq!(structure.potential_indices, vec![0, 2, 1, 1]);
    }

    #[test]
    fn parser_rejects_unknown_atomic_symbols() {
        let input = "atomnum\n1\n1\ntitle\nXx 0.0 0.0 0.0\n";
        let error = parse_pot_generator_test_input(input).expect_err("symbol should be invalid");
        assert_eq!(
            error,
            PotGeneratorTestError::UnknownAtomicSymbol("Xx".to_string())
        );
    }
}
