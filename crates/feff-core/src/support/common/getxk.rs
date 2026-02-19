pub fn getxk(energy_hartree: f64) -> f64 {
    let magnitude = (2.0 * energy_hartree).abs().sqrt();
    if energy_hartree < 0.0 {
        -magnitude
    } else {
        magnitude
    }
}

#[cfg(test)]
mod tests {
    use super::getxk;

    #[test]
    fn getxk_returns_positive_k_for_positive_energy() {
        let k = getxk(2.0);
        assert!((k - 2.0).abs() < 1.0e-12);
    }

    #[test]
    fn getxk_returns_negative_k_for_negative_energy() {
        let k = getxk(-2.0);
        assert!((k + 2.0).abs() < 1.0e-12);
    }

    #[test]
    fn getxk_returns_zero_for_zero_energy() {
        assert_eq!(getxk(0.0), 0.0);
    }
}
