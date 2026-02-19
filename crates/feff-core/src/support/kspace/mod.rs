pub mod cgcrac;
pub mod strconfra;
pub mod strfunqjl;

pub fn factorial_table(max: usize) -> Vec<f64> {
    let mut values = vec![1.0_f64; max + 1];
    for index in 1..=max {
        values[index] = values[index - 1] * index as f64;
    }
    values
}

#[cfg(test)]
mod tests {
    use super::factorial_table;

    #[test]
    fn factorial_table_contains_expected_prefix() {
        let values = factorial_table(6);
        let expected = vec![1.0, 1.0, 2.0, 6.0, 24.0, 120.0, 720.0];
        assert_eq!(values, expected);
    }
}
