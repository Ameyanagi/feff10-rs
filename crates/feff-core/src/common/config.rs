//! FEFF COMMON electronic-configuration dataset and lookup helpers.
//!
//! Data is ported from `feff10/src/COMMON/m_config.f90` and exposed through
//! atomic-number keyed APIs for physics modules (POT now, ATOM in follow-up stories).

use super::config_data::{
    ELEMENT_SYMBOLS, FEFF7_OCCUPATION, FEFF7_VALENCE, FEFF9_OCCUPATION, FEFF9_SPIN, FEFF9_VALENCE,
    MAX_ATOMIC_NUMBER, NOBLE_GAS_ATOMIC_NUMBERS, NOBLE_GAS_COUNT, ORBITAL_COUNT, ORBITAL_KAPPA,
    ORBITAL_NNUM,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigurationRecipe {
    Feff9,
    Feff7,
}

#[derive(Debug, Clone, Copy)]
pub struct ElectronicConfiguration {
    occupations: &'static [f64; ORBITAL_COUNT],
    valence: &'static [f64; ORBITAL_COUNT],
    spin: &'static [f64; ORBITAL_COUNT],
}

impl ElectronicConfiguration {
    pub fn occupations(&self) -> &'static [f64; ORBITAL_COUNT] {
        self.occupations
    }

    pub fn valence(&self) -> &'static [f64; ORBITAL_COUNT] {
        self.valence
    }

    pub fn spin(&self) -> &'static [f64; ORBITAL_COUNT] {
        self.spin
    }

    pub fn total_occupation(&self) -> f64 {
        self.occupations.iter().sum()
    }

    pub fn total_valence(&self) -> f64 {
        self.valence.iter().sum()
    }

    pub fn occupied_orbital_count(&self) -> usize {
        self.occupations
            .iter()
            .filter(|occupation| **occupation > 0.0)
            .count()
    }
}

const FEFF7_SPIN: [f64; ORBITAL_COUNT] = [0.0; ORBITAL_COUNT];

pub fn configuration_for_atomic_number(
    atomic_number: usize,
    recipe: ConfigurationRecipe,
) -> Option<ElectronicConfiguration> {
    let index = index_for_atomic_number(atomic_number)?;

    let configuration = match recipe {
        ConfigurationRecipe::Feff9 => ElectronicConfiguration {
            occupations: &FEFF9_OCCUPATION[index],
            valence: &FEFF9_VALENCE[index],
            spin: &FEFF9_SPIN[index],
        },
        ConfigurationRecipe::Feff7 => ElectronicConfiguration {
            occupations: &FEFF7_OCCUPATION[index],
            valence: &FEFF7_VALENCE[index],
            spin: &FEFF7_SPIN,
        },
    };

    Some(configuration)
}

pub fn feff9_for_atomic_number(atomic_number: usize) -> Option<ElectronicConfiguration> {
    configuration_for_atomic_number(atomic_number, ConfigurationRecipe::Feff9)
}

pub fn feff7_for_atomic_number(atomic_number: usize) -> Option<ElectronicConfiguration> {
    configuration_for_atomic_number(atomic_number, ConfigurationRecipe::Feff7)
}

pub fn element_symbol(atomic_number: usize) -> Option<&'static str> {
    let index = index_for_atomic_number(atomic_number)?;
    Some(ELEMENT_SYMBOLS[index])
}

pub fn atomic_number_for_symbol(symbol: &str) -> Option<usize> {
    let normalized = symbol.trim();
    if normalized.is_empty() {
        return None;
    }

    ELEMENT_SYMBOLS
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(normalized))
        .map(|index| index + 1)
}

pub fn orbital_kappa_quantum_numbers() -> &'static [i32; ORBITAL_COUNT] {
    &ORBITAL_KAPPA
}

pub fn orbital_principal_quantum_numbers() -> &'static [i32; ORBITAL_COUNT] {
    &ORBITAL_NNUM
}

pub fn noble_gas_atomic_numbers() -> &'static [usize; NOBLE_GAS_COUNT] {
    &NOBLE_GAS_ATOMIC_NUMBERS
}

const fn index_for_atomic_number(atomic_number: usize) -> Option<usize> {
    if atomic_number == 0 || atomic_number > MAX_ATOMIC_NUMBER {
        None
    } else {
        Some(atomic_number - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        atomic_number_for_symbol, configuration_for_atomic_number, element_symbol,
        feff7_for_atomic_number, feff9_for_atomic_number, noble_gas_atomic_numbers,
        orbital_kappa_quantum_numbers, orbital_principal_quantum_numbers, ConfigurationRecipe,
        MAX_ATOMIC_NUMBER, ORBITAL_COUNT,
    };

    #[test]
    fn lookup_rejects_out_of_range_atomic_numbers() {
        assert!(configuration_for_atomic_number(0, ConfigurationRecipe::Feff9).is_none());
        assert!(configuration_for_atomic_number(140, ConfigurationRecipe::Feff7).is_none());
        assert!(element_symbol(0).is_none());
        assert!(element_symbol(140).is_none());
    }

    #[test]
    fn known_symbol_roundtrip_matches_atomic_number() {
        assert_eq!(atomic_number_for_symbol("Cu"), Some(29));
        assert_eq!(atomic_number_for_symbol("cu"), Some(29));
        assert_eq!(atomic_number_for_symbol(" U "), Some(92));
        assert_eq!(element_symbol(118), Some("Uuo"));
        assert_eq!(element_symbol(139), Some("Ute"));
        assert_eq!(atomic_number_for_symbol(""), None);
        assert_eq!(atomic_number_for_symbol("Xx"), None);
    }

    #[test]
    fn feff9_profiles_match_reference_rows() {
        let carbon = feff9_for_atomic_number(6).expect("carbon configuration should exist");
        assert_eq!(carbon.occupations()[..6], [2.0, 1.0, 2.0, 1.0, 0.0, 0.0]);
        assert_eq!(carbon.valence()[..6], [0.0, 1.0, 2.0, 1.0, 0.0, 0.0]);
        assert_eq!(carbon.spin()[..6], [0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);

        let copper = feff9_for_atomic_number(29).expect("copper configuration should exist");
        assert_eq!(
            copper.occupations()[..12],
            [2.0, 2.0, 2.0, 4.0, 2.0, 2.0, 4.0, 4.0, 6.0, 1.0, 0.0, 0.0]
        );
        assert_eq!(copper.total_occupation(), 29.0);
        assert_eq!(copper.total_valence(), 11.0);
    }

    #[test]
    fn feff7_spin_channel_is_zeroed() {
        let uranium = feff7_for_atomic_number(92).expect("uranium configuration should exist");
        assert!(uranium.spin().iter().all(|value| *value == 0.0));
    }

    #[test]
    fn feff9_total_occupation_matches_atomic_number_for_all_elements() {
        for atomic_number in 1..=MAX_ATOMIC_NUMBER {
            let configuration = feff9_for_atomic_number(atomic_number)
                .expect("configuration should exist for in-range z");
            let expected = atomic_number as f64;
            assert!(
                (configuration.total_occupation() - expected).abs() <= 1.0e-9,
                "z={} total occupation was {}",
                atomic_number,
                configuration.total_occupation()
            );
        }
    }

    #[test]
    fn orbital_metadata_lengths_and_noble_gas_rows_match_feff_tables() {
        let nnum = orbital_principal_quantum_numbers();
        let kappa = orbital_kappa_quantum_numbers();
        let nobles = noble_gas_atomic_numbers();

        assert_eq!(nnum.len(), ORBITAL_COUNT);
        assert_eq!(kappa.len(), ORBITAL_COUNT);
        assert_eq!(nobles, &[2, 10, 18, 36, 54, 80, 86, 118]);
        assert_eq!(nnum[0], 1);
        assert_eq!(kappa[0], -1);
        assert_eq!(nnum[39], 5);
        assert_eq!(kappa[39], -5);
    }
}
