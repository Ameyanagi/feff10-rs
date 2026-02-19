pub fn aprdep(a: &[f64], b: &[f64], l: usize) -> f64 {
    if l == 0 || a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let upper = l.min(a.len()).min(b.len());
    let mut value = 0.0_f64;
    for m in 0..upper {
        value += a[m] * b[upper - 1 - m];
    }
    value
}

#[cfg(test)]
mod tests {
    use super::aprdep;

    #[test]
    fn aprdep_matches_polynomial_convolution_coefficient() {
        let a = [1.0_f64, 2.0, 3.0];
        let b = [4.0_f64, 5.0, 6.0];
        let value = aprdep(&a, &b, 3);
        assert!((value - (1.0 * 6.0 + 2.0 * 5.0 + 3.0 * 4.0)).abs() <= 1.0e-12);
    }

    #[test]
    fn aprdep_zero_order_is_zero() {
        assert_eq!(aprdep(&[1.0], &[2.0], 0), 0.0);
    }

    #[test]
    fn aprdep_truncates_to_shortest_input() {
        let value = aprdep(&[3.0, 2.0], &[5.0], 4);
        assert_eq!(value, 15.0);
    }
}
