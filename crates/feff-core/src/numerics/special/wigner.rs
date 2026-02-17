#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wigner3jInput {
    pub two_j1: i32,
    pub two_j2: i32,
    pub two_j3: i32,
    pub two_m1: i32,
    pub two_m2: i32,
    pub two_m3: i32,
}

impl Wigner3jInput {
    pub fn new(
        two_j1: i32,
        two_j2: i32,
        two_j3: i32,
        two_m1: i32,
        two_m2: i32,
        two_m3: i32,
    ) -> Self {
        Self {
            two_j1,
            two_j2,
            two_j3,
            two_m1,
            two_m2,
            two_m3,
        }
    }
}

/// Computes the Wigner 3j coefficient using doubled quantum numbers.
///
/// All `two_*` values represent `2*j` or `2*m` (e.g., `two_j=3` means `j=3/2`).
pub fn wigner_3j(input: Wigner3jInput) -> f64 {
    let Wigner3jInput {
        two_j1,
        two_j2,
        two_j3,
        two_m1,
        two_m2,
        two_m3,
    } = input;

    if two_j1 < 0 || two_j2 < 0 || two_j3 < 0 {
        return 0.0;
    }

    if two_m1 + two_m2 + two_m3 != 0 {
        return 0.0;
    }

    if two_m1.abs() > two_j1 || two_m2.abs() > two_j2 || two_m3.abs() > two_j3 {
        return 0.0;
    }

    if (two_j1 - two_m1).rem_euclid(2) != 0
        || (two_j2 - two_m2).rem_euclid(2) != 0
        || (two_j3 - two_m3).rem_euclid(2) != 0
    {
        return 0.0;
    }

    if (two_j1 + two_j2 + two_j3).rem_euclid(2) != 0 {
        return 0.0;
    }

    if two_j1 + two_j2 < two_j3 || two_j1 + two_j3 < two_j2 || two_j2 + two_j3 < two_j1 {
        return 0.0;
    }

    // FEFF cwig3j.f90 logic with ient=2 (semiinteger mode).
    let mut terms = [
        two_j1 + two_j2 - two_j3,
        two_j2 + two_j3 - two_j1,
        two_j3 + two_j1 - two_j2,
        two_j1 + two_m1,
        two_j1 - two_m1,
        two_j2 + two_m2,
        two_j2 - two_m2,
        two_j3 + two_m3,
        two_j3 - two_m3,
        two_j1 + two_j2 + two_j3 + 2,
        two_j2 - two_j3 - two_m1,
        two_j1 - two_j3 + two_m2,
    ];

    for (index, term) in terms.iter_mut().enumerate() {
        if index < 10 && *term < 0 {
            return 0.0;
        }

        if term.rem_euclid(2) != 0 {
            return 0.0;
        }

        *term /= 2;
    }

    let max0 = terms[10].max(terms[11]).max(0) + 1;
    let min0 = terms[0].min(terms[4]).min(terms[5]) + 1;
    if max0 > min0 {
        return 0.0;
    }

    let mut log_factorial = LogFactorial::new();
    let mut prefactor_log = -log_factorial.value((terms[9] + 1) as usize);
    for value in terms.iter().take(9) {
        prefactor_log += log_factorial.value((*value + 1) as usize);
    }
    prefactor_log *= 0.5;

    let mut sign = if (max0 - 1).rem_euclid(2) != 0 {
        -1.0
    } else {
        1.0
    };
    let mut result = 0.0;
    for i in max0..=min0 {
        let j = 2 - i;
        let denominator_log = log_factorial.value(i as usize)
            + log_factorial.value((j + terms[0]) as usize)
            + log_factorial.value((j + terms[4]) as usize)
            + log_factorial.value((j + terms[5]) as usize)
            + log_factorial.value((i - terms[10]) as usize)
            + log_factorial.value((i - terms[11]) as usize);

        result += sign * (prefactor_log - denominator_log).exp();
        sign = -sign;
    }

    if (two_j1 - two_j2 - two_m3).rem_euclid(4) != 0 {
        result = -result;
    }

    result
}

#[derive(Default)]
struct LogFactorial {
    values: Vec<f64>,
}

impl LogFactorial {
    fn new() -> Self {
        Self { values: vec![0.0] }
    }

    fn value(&mut self, feff_index: usize) -> f64 {
        assert!(feff_index >= 1, "FEFF factorial index must be >= 1");
        let factorial_n = feff_index - 1;

        while self.values.len() <= factorial_n {
            let next_index = self.values.len();
            let next_value = self.values[next_index - 1] + (next_index as f64).ln();
            self.values.push(next_value);
        }

        self.values[factorial_n]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wigner6jInput {
    pub two_j1: i32,
    pub two_j2: i32,
    pub two_j3: i32,
    pub two_j4: i32,
    pub two_j5: i32,
    pub two_j6: i32,
}

impl Wigner6jInput {
    pub fn new(
        two_j1: i32,
        two_j2: i32,
        two_j3: i32,
        two_j4: i32,
        two_j5: i32,
        two_j6: i32,
    ) -> Self {
        Self {
            two_j1,
            two_j2,
            two_j3,
            two_j4,
            two_j5,
            two_j6,
        }
    }
}

pub trait WignerSymbolsApi {
    fn wigner_3j(&self, input: Wigner3jInput) -> f64;
    fn wigner_6j(&self, input: Wigner6jInput) -> f64;
}

#[cfg(test)]
mod tests {
    use super::{wigner_3j, Wigner3jInput};
    use std::f64::consts::FRAC_1_SQRT_2;

    #[test]
    fn wigner_3j_returns_zero_for_selection_rule_violations() {
        let cases = [
            Wigner3jInput::new(2, 2, 0, 0, 0, 2),  // m1 + m2 + m3 != 0
            Wigner3jInput::new(2, 2, 8, 0, 0, 0),  // triangle inequality violation
            Wigner3jInput::new(2, 2, 0, 4, -4, 0), // |m1| > j1
            Wigner3jInput::new(1, 1, 1, 1, -1, 0), // j1 + j2 + j3 not integer
            Wigner3jInput::new(2, 2, 2, 1, -1, 0), // parity mismatch between j and m
        ];

        for input in cases {
            let actual = wigner_3j(input);
            assert!(
                actual.abs() <= 1.0e-15,
                "selection-rule violation should return 0, got {actual:.16e} for {:?}",
                input
            );
        }
    }

    #[test]
    fn wigner_3j_matches_tabulated_reference_values() {
        // Reference values generated with FEFF cwig3j.f90.
        let cases = [
            ("j=0,m=0", Wigner3jInput::new(0, 0, 0, 0, 0, 0), 1.0),
            (
                "(1,1,0;0,0,0)",
                Wigner3jInput::new(2, 2, 0, 0, 0, 0),
                -1.0 / 3.0_f64.sqrt(),
            ),
            (
                "(1,1,2;0,0,0)",
                Wigner3jInput::new(2, 2, 4, 0, 0, 0),
                (2.0_f64 / 15.0_f64).sqrt(),
            ),
            (
                "(1/2,1/2,0;1/2,-1/2,0)",
                Wigner3jInput::new(1, 1, 0, 1, -1, 0),
                FRAC_1_SQRT_2,
            ),
            (
                "(1/2,1/2,1;1/2,1/2,-1)",
                Wigner3jInput::new(1, 1, 2, 1, 1, -2),
                -1.0 / 3.0_f64.sqrt(),
            ),
            (
                "(3/2,1,1/2;1/2,0,-1/2)",
                Wigner3jInput::new(3, 2, 1, 1, 0, -1),
                1.0 / 6.0_f64.sqrt(),
            ),
            (
                "(3/2,1,1/2;-1/2,0,1/2)",
                Wigner3jInput::new(3, 2, 1, -1, 0, 1),
                -1.0 / 6.0_f64.sqrt(),
            ),
        ];

        for (label, input, expected) in cases {
            let actual = wigner_3j(input);
            assert_scalar_close(label, expected, actual, 1.0e-15, 1.0e-14);
        }
    }

    fn assert_scalar_close(label: &str, expected: f64, actual: f64, abs_tol: f64, rel_tol: f64) {
        let abs_diff = (actual - expected).abs();
        let rel_diff = abs_diff / expected.abs().max(1.0);
        assert!(
            abs_diff <= abs_tol || rel_diff <= rel_tol,
            "{label} expected={expected:.15e} actual={actual:.15e} abs_diff={abs_diff:.15e} rel_diff={rel_diff:.15e} abs_tol={abs_tol:.15e} rel_tol={rel_tol:.15e}",
        );
    }
}
