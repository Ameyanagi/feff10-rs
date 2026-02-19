const ATOMIC_WEIGHTS: [f64; 139] = [
    1.0079_f64,
    4.0026_f64,
    6.941_f64,
    9.0122_f64,
    10.81_f64,
    12.01_f64,
    14.007_f64,
    15.999_f64,
    18.998_f64,
    20.18_f64,
    22.9898_f64,
    24.305_f64,
    26.982_f64,
    28.086_f64,
    30.974_f64,
    32.064_f64,
    35.453_f64,
    39.948_f64,
    39.09_f64,
    40.08_f64,
    44.956_f64,
    47.90_f64,
    50.942_f64,
    52.00_f64,
    54.938_f64,
    55.85_f64,
    58.93_f64,
    58.71_f64,
    63.55_f64,
    65.38_f64,
    69.72_f64,
    72.59_f64,
    74.922_f64,
    78.96_f64,
    79.91_f64,
    83.80_f64,
    85.47_f64,
    87.62_f64,
    88.91_f64,
    91.22_f64,
    92.91_f64,
    95.94_f64,
    98.91_f64,
    101.07_f64,
    102.90_f64,
    106.40_f64,
    107.87_f64,
    112.40_f64,
    114.82_f64,
    118.69_f64,
    121.75_f64,
    127.60_f64,
    126.90_f64,
    131.30_f64,
    132.91_f64,
    137.34_f64,
    138.91_f64,
    140.12_f64,
    140.91_f64,
    144.24_f64,
    145_f64,
    150.35_f64,
    151.96_f64,
    157.25_f64,
    158.92_f64,
    162.50_f64,
    164.93_f64,
    167.26_f64,
    168.93_f64,
    173.04_f64,
    174.97_f64,
    178.49_f64,
    180.95_f64,
    183.85_f64,
    186.2_f64,
    190.20_f64,
    192.22_f64,
    195.09_f64,
    196.97_f64,
    200.59_f64,
    204.37_f64,
    207.19_f64,
    208.98_f64,
    210_f64,
    210_f64,
    222_f64,
    223_f64,
    226_f64,
    227_f64,
    232.04_f64,
    231_f64,
    238.03_f64,
    237.05_f64,
    244_f64,
    243_f64,
    247_f64,
    247_f64,
    251_f64,
    252_f64,
    257_f64,
    258_f64,
    259_f64,
    266_f64,
    267_f64,
    268_f64,
    269_f64,
    270_f64,
    269_f64,
    278_f64,
    281_f64,
    282_f64,
    285_f64,
    286_f64,
    289_f64,
    289_f64,
    293_f64,
    294_f64,
    294_f64,
    315_f64,
    320_f64,
    330_f64,
    334_f64,
    337_f64,
    340_f64,
    344_f64,
    347_f64,
    350_f64,
    354_f64,
    357_f64,
    361_f64,
    364_f64,
    367_f64,
    371_f64,
    374_f64,
    378_f64,
    381_f64,
    385_f64,
    388_f64,
    392_f64,
];

const ATOMIC_SYMBOLS: [&str; 139] = [
    "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S", "Cl",
    "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga", "Ge", "As",
    "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd", "Ag", "Cd", "In",
    "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd", "Pm", "Sm", "Eu", "Gd", "Tb",
    "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Te", "Os", "Ir", "Pt", "Au", "Hg", "Tl",
    "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa", "U", "Np", "Pu", "Am", "Cm", "Bk",
    "Cf", "Es", "Fm", "Md", "No", "Lr", "Rf", "Db", "Sg", "Bh", "Hs", "Mt", "Ds", "Rg", "Cn",
    "Uut", "Fl", "Uup", "Lv", "Uus", "Uuo", "Uue", "Ubn", "Ubu", "Ubb", "Ubt", "Ubq", "Ubp", "Ubh",
    "Ubs", "Ubo", "Ube", "Utn", "Utu", "Utb", "Utt", "Utq", "Utp", "Uth", "Uts", "Uto", "Ute",
];

pub fn atwtd(iz: usize) -> Option<f64> {
    index_1_based(iz).map(|index| ATOMIC_WEIGHTS[index])
}

pub fn atwts(iz: usize) -> Option<f32> {
    atwtd(iz).map(|weight| weight as f32)
}

pub fn atsym(iz: usize) -> Option<&'static str> {
    index_1_based(iz).map(|index| ATOMIC_SYMBOLS[index])
}

fn index_1_based(iz: usize) -> Option<usize> {
    if iz == 0 || iz > ATOMIC_WEIGHTS.len() {
        None
    } else {
        Some(iz - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::{atsym, atwtd, atwts};

    #[test]
    fn periodic_table_weight_lookup_is_one_based() {
        assert_eq!(atwtd(1), Some(1.0079));
        assert_eq!(atwtd(29), Some(63.55));
        assert_eq!(atwtd(139), Some(392.0));
        assert_eq!(atwtd(0), None);
        assert_eq!(atwtd(140), None);
    }

    #[test]
    fn periodic_table_symbol_lookup_is_one_based() {
        assert_eq!(atsym(1), Some("H"));
        assert_eq!(atsym(29), Some("Cu"));
        assert_eq!(atsym(92), Some("U"));
        assert_eq!(atsym(0), None);
    }

    #[test]
    fn single_precision_weight_conversion_matches_double_precision_table() {
        let single = atwts(47).expect("Ag weight should exist");
        assert!((single as f64 - 107.87).abs() < 1.0e-5);
    }
}
