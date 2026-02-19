pub fn aprdev(a: &[f64], b: &[f64], l: usize) -> f64 {
    if l == 0 || a.is_empty() || b.is_empty() {
        return 0.0;
    }

    let upper = l.min(a.len()).min(b.len());
    let mut value = 0.0;
    for m in 0..upper {
        value += a[m] * b[upper - 1 - m];
    }
    value
}

#[cfg(test)]
mod tests {
    use super::aprdev;

    #[test]
    fn aprdev_matches_convolution_coefficient() {
        let a = [1.0, 2.0, 3.0];
        let b = [4.0, 5.0, 6.0];
        let value = aprdev(&a, &b, 3);
        assert!((value - 28.0).abs() <= 1.0e-12);
    }

    #[test]
    fn aprdev_zero_order_is_zero() {
        assert_eq!(aprdev(&[1.0], &[2.0], 0), 0.0);
    }

    #[test]
    fn aprdev_truncates_to_available_coefficients() {
        let value = aprdev(&[3.0, 2.0], &[5.0], 4);
        assert_eq!(value, 15.0);
    }
}
