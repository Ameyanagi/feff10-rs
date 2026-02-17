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

const MIN_KAPPA_PROJECTION: i32 = -5;
const MAX_KAPPA_PROJECTION: i32 = 4;
const KAPPA_PROJECTION_SLOTS: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElectronShell {
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
}

impl ElectronShell {
    pub const fn principal_quantum_number(self) -> i32 {
        match self {
            Self::K => 1,
            Self::L => 2,
            Self::M => 3,
            Self::N => 4,
            Self::O => 5,
            Self::P => 6,
            Self::Q => 7,
            Self::R => 8,
        }
    }

    pub const fn from_principal_quantum_number(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::K),
            2 => Some(Self::L),
            3 => Some(Self::M),
            4 => Some(Self::N),
            5 => Some(Self::O),
            6 => Some(Self::P),
            7 => Some(Self::Q),
            8 => Some(Self::R),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrbitalMetadata {
    pub orbital_index: usize,
    pub principal_quantum_number: i32,
    pub kappa_quantum_number: i32,
    pub shell: ElectronShell,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitalOccupancy {
    pub metadata: OrbitalMetadata,
    pub occupation: f64,
    pub valence_occupation: f64,
    pub spin_occupation: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrbitalExtraction {
    orbitals: Vec<OrbitalOccupancy>,
    projection_last_occupied_orbital: [Option<usize>; KAPPA_PROJECTION_SLOTS],
}

impl OrbitalExtraction {
    pub fn orbitals(&self) -> &[OrbitalOccupancy] {
        &self.orbitals
    }

    pub fn orbitals_in_shell(&self, shell: ElectronShell) -> Vec<OrbitalOccupancy> {
        self.orbitals
            .iter()
            .copied()
            .filter(|orbital| orbital.metadata.shell == shell)
            .collect()
    }

    pub fn projection_orbital_index_for_kappa(&self, kappa_quantum_number: i32) -> Option<usize> {
        let index = projection_index_for_kappa(kappa_quantum_number)?;
        self.projection_last_occupied_orbital[index]
    }
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

pub fn orbital_metadata(orbital_index: usize) -> Option<OrbitalMetadata> {
    if orbital_index == 0 || orbital_index > ORBITAL_COUNT {
        return None;
    }

    let table_index = orbital_index - 1;
    let principal_quantum_number = ORBITAL_NNUM[table_index];
    let shell = ElectronShell::from_principal_quantum_number(principal_quantum_number)?;

    Some(OrbitalMetadata {
        orbital_index,
        principal_quantum_number,
        kappa_quantum_number: ORBITAL_KAPPA[table_index],
        shell,
    })
}

pub fn orbital_occupancy_for_atomic_number(
    atomic_number: usize,
    recipe: ConfigurationRecipe,
    orbital_index: usize,
) -> Option<OrbitalOccupancy> {
    let configuration = configuration_for_atomic_number(atomic_number, recipe)?;
    let metadata = orbital_metadata(orbital_index)?;
    let table_index = orbital_index - 1;

    Some(OrbitalOccupancy {
        metadata,
        occupation: configuration.occupations()[table_index],
        valence_occupation: configuration.valence()[table_index],
        spin_occupation: configuration.spin()[table_index],
    })
}

pub fn getorb_for_atomic_number(
    atomic_number: usize,
    recipe: ConfigurationRecipe,
) -> Option<OrbitalExtraction> {
    let configuration = configuration_for_atomic_number(atomic_number, recipe)?;
    let mut orbitals = Vec::new();
    let mut projection_last_occupied_orbital = [None; KAPPA_PROJECTION_SLOTS];

    for orbital_index in 1..=ORBITAL_COUNT {
        let table_index = orbital_index - 1;
        let occupation = configuration.occupations()[table_index];
        if occupation <= 0.0 {
            continue;
        }

        let metadata = orbital_metadata(orbital_index)?;
        if let Some(index) = projection_index_for_kappa(metadata.kappa_quantum_number) {
            projection_last_occupied_orbital[index] = Some(orbital_index);
        }

        orbitals.push(OrbitalOccupancy {
            metadata,
            occupation,
            valence_occupation: configuration.valence()[table_index],
            spin_occupation: configuration.spin()[table_index],
        });
    }

    Some(OrbitalExtraction {
        orbitals,
        projection_last_occupied_orbital,
    })
}

pub fn shell_orbitals_for_atomic_number(
    atomic_number: usize,
    recipe: ConfigurationRecipe,
    shell: ElectronShell,
) -> Option<Vec<OrbitalOccupancy>> {
    let extraction = getorb_for_atomic_number(atomic_number, recipe)?;
    Some(extraction.orbitals_in_shell(shell))
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

const fn projection_index_for_kappa(kappa_quantum_number: i32) -> Option<usize> {
    if kappa_quantum_number < MIN_KAPPA_PROJECTION || kappa_quantum_number > MAX_KAPPA_PROJECTION {
        None
    } else {
        Some((kappa_quantum_number - MIN_KAPPA_PROJECTION) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        atomic_number_for_symbol, configuration_for_atomic_number, element_symbol,
        feff7_for_atomic_number, feff9_for_atomic_number, getorb_for_atomic_number,
        noble_gas_atomic_numbers, orbital_kappa_quantum_numbers, orbital_metadata,
        orbital_principal_quantum_numbers, shell_orbitals_for_atomic_number, ConfigurationRecipe,
        ElectronShell, MAX_ATOMIC_NUMBER, ORBITAL_COUNT,
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

    #[test]
    fn getorb_metadata_reports_quantum_numbers_and_shell() {
        let one_s = orbital_metadata(1).expect("1s metadata should be available");
        assert_eq!(one_s.principal_quantum_number, 1);
        assert_eq!(one_s.kappa_quantum_number, -1);
        assert_eq!(one_s.shell, ElectronShell::K);

        let five_g = orbital_metadata(39).expect("5g7/2 metadata should be available");
        assert_eq!(five_g.principal_quantum_number, 5);
        assert_eq!(five_g.kappa_quantum_number, 4);
        assert_eq!(five_g.shell, ElectronShell::O);
    }

    #[test]
    fn getorb_shell_lookup_supports_k_l_and_m_shell_queries() {
        let carbon_l_shell =
            shell_orbitals_for_atomic_number(6, ConfigurationRecipe::Feff9, ElectronShell::L)
                .expect("carbon L-shell lookup should succeed");
        let carbon_l_indices: Vec<usize> = carbon_l_shell
            .iter()
            .map(|orbital| orbital.metadata.orbital_index)
            .collect();
        let carbon_l_occupancies: Vec<f64> = carbon_l_shell
            .iter()
            .map(|orbital| orbital.occupation)
            .collect();
        assert_eq!(carbon_l_indices, vec![2, 3, 4]);
        assert_eq!(carbon_l_occupancies, vec![1.0, 2.0, 1.0]);

        let copper_k_shell =
            shell_orbitals_for_atomic_number(29, ConfigurationRecipe::Feff9, ElectronShell::K)
                .expect("copper K-shell lookup should succeed");
        assert_eq!(copper_k_shell.len(), 1);
        assert_eq!(copper_k_shell[0].metadata.orbital_index, 1);
        assert_eq!(copper_k_shell[0].occupation, 2.0);

        let krypton_m_shell =
            shell_orbitals_for_atomic_number(36, ConfigurationRecipe::Feff9, ElectronShell::M)
                .expect("krypton M-shell lookup should succeed");
        let krypton_m_indices: Vec<usize> = krypton_m_shell
            .iter()
            .map(|orbital| orbital.metadata.orbital_index)
            .collect();
        let krypton_m_occupancies: Vec<f64> = krypton_m_shell
            .iter()
            .map(|orbital| orbital.occupation)
            .collect();
        assert_eq!(krypton_m_indices, vec![5, 6, 7, 8, 9]);
        assert_eq!(krypton_m_occupancies, vec![2.0, 2.0, 4.0, 4.0, 6.0]);
    }

    #[test]
    fn getorb_projection_lookup_matches_last_occupied_kappa_orbitals() {
        let extraction = getorb_for_atomic_number(29, ConfigurationRecipe::Feff9)
            .expect("copper extraction should succeed");

        assert_eq!(extraction.projection_orbital_index_for_kappa(-3), Some(9));
        assert_eq!(extraction.projection_orbital_index_for_kappa(-1), Some(10));
        assert_eq!(extraction.projection_orbital_index_for_kappa(1), Some(6));
        assert_eq!(extraction.projection_orbital_index_for_kappa(4), None);
        assert_eq!(extraction.projection_orbital_index_for_kappa(5), None);
    }
}
