use num_complex::Complex64;

pub fn aprdec(ala: &[Complex64], bla: &[Complex64], l: usize) -> Complex64 {
    if l == 0 || ala.is_empty() || bla.is_empty() {
        return Complex64::new(0.0, 0.0);
    }

    let upper = l.min(ala.len()).min(bla.len());
    let mut value = Complex64::new(0.0, 0.0);
    for m in 0..upper {
        value += ala[m] * bla[upper - 1 - m];
    }
    value
}

#[cfg(test)]
mod tests {
    use super::aprdec;
    use num_complex::Complex64;

    fn c(re: f64, im: f64) -> Complex64 {
        Complex64::new(re, im)
    }

    #[test]
    fn aprdec_matches_convolution_coefficient() {
        let a = [c(1.0, 0.0), c(2.0, -1.0), c(0.5, 0.25)];
        let b = [c(-3.0, 0.5), c(4.0, 1.0), c(2.0, -2.0)];
        let value = aprdec(&a, &b, 3);
        let expected = a[0] * b[2] + a[1] * b[1] + a[2] * b[0];
        assert!((value - expected).norm() <= 1.0e-12);
    }

    #[test]
    fn aprdec_zero_order_is_zero() {
        let value = aprdec(&[c(1.0, 0.0)], &[c(2.0, 0.0)], 0);
        assert!((value - c(0.0, 0.0)).norm() <= 1.0e-12);
    }

    #[test]
    fn aprdec_truncates_to_available_coefficients() {
        let a = [c(2.0, 0.0), c(1.0, 0.0)];
        let b = [c(1.0, 0.0)];
        let value = aprdec(&a, &b, 3);
        assert!((value - c(2.0, 0.0)).norm() <= 1.0e-12);
    }
}
