pub fn dentfa(dr: f64, dz: f64, ch: f64) -> f64 {
    if dr <= 0.0 {
        return 0.0;
    }

    let effective_charge = dz + ch;
    if effective_charge < 1.0e-4 {
        return 0.0;
    }

    let mut w = dr * effective_charge.powf(1.0 / 3.0);
    w = (w / 0.8853).sqrt();
    let t = w * (0.60112 * w + 1.81061) + 1.0;
    let denominator =
        w * (w * (w * (w * (0.04793 * w + 0.21465) + 0.77112) + 1.39515) + 1.81061) + 1.0;
    effective_charge * (1.0 - (t / denominator).powi(2)) / dr
}

#[cfg(test)]
mod tests {
    use super::dentfa;

    #[test]
    fn dentfa_returns_zero_for_non_positive_radius() {
        assert_eq!(dentfa(0.0, 8.0, 0.0), 0.0);
    }

    #[test]
    fn dentfa_returns_zero_for_nearly_empty_effective_charge() {
        assert_eq!(dentfa(1.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn dentfa_matches_reference_value_for_neutral_oxygen() {
        let value = dentfa(1.0, 8.0, 0.0);
        assert!((value - 6.280_065_287_728_364).abs() <= 1.0e-12);
    }
}
