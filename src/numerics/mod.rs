pub fn stable_sum(values: &[f64]) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;

    for &value in values {
        let corrected = value - correction;
        let next = sum + corrected;
        correction = (next - sum) - corrected;
        sum = next;
    }

    sum
}

pub fn relative_difference(lhs: f64, rhs: f64, relative_floor: f64) -> f64 {
    let scale = lhs.abs().max(rhs.abs()).max(relative_floor);
    (lhs - rhs).abs() / scale
}

pub fn within_tolerance(
    lhs: f64,
    rhs: f64,
    abs_tol: f64,
    rel_tol: f64,
    relative_floor: f64,
) -> bool {
    let abs_diff = (lhs - rhs).abs();
    abs_diff <= abs_tol || relative_difference(lhs, rhs, relative_floor) <= rel_tol
}

#[cfg(test)]
mod tests {
    use super::{relative_difference, stable_sum, within_tolerance};

    #[test]
    fn stable_sum_reduces_order_loss_for_large_and_small_values() {
        let input = [1.0e16, 1.0, -1.0e16];
        assert_eq!(stable_sum(&input), 0.0);
    }

    #[test]
    fn relative_difference_uses_relative_floor() {
        let diff = relative_difference(0.0, 1.0e-10, 1.0e-6);
        assert!((diff - 1.0e-4).abs() < 1.0e-12);
    }

    #[test]
    fn within_tolerance_accepts_abs_or_relative_match() {
        assert!(within_tolerance(10.0, 10.001, 1.0e-2, 1.0e-6, 1.0e-12));
        assert!(within_tolerance(1000.0, 1000.2, 1.0e-6, 5.0e-4, 1.0e-12));
        assert!(!within_tolerance(1.0, 1.1, 1.0e-3, 1.0e-3, 1.0e-12));
    }
}
