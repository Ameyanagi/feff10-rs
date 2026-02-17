use crate::common::constants::PI;
use num_complex::Complex64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphericalHarmonicsInput {
    pub degree: i32,
    pub order: i32,
    pub theta: f64,
    pub phi: f64,
}

impl SphericalHarmonicsInput {
    pub fn new(degree: i32, order: i32, theta: f64, phi: f64) -> Self {
        Self {
            degree,
            order,
            theta,
            phi,
        }
    }
}

pub trait SphericalHarmonicsApi {
    fn y_lm(&self, input: SphericalHarmonicsInput) -> Complex64;
}

pub fn y_lm(degree: i32, order: i32, theta: f64, phi: f64) -> Complex64 {
    assert!(degree >= 0, "spherical harmonics requires degree >= 0");
    assert!(
        order.abs() <= degree,
        "spherical harmonics requires |order| <= degree"
    );

    let degree = degree as usize;
    if order >= 0 {
        return y_lm_nonnegative_order(degree, order as usize, theta, phi);
    }

    let positive_order = (-order) as usize;
    let positive = y_lm_nonnegative_order(degree, positive_order, theta, phi);
    if positive_order % 2 == 0 {
        positive.conj()
    } else {
        -positive.conj()
    }
}

pub fn spherical_y(input: SphericalHarmonicsInput) -> Complex64 {
    y_lm(input.degree, input.order, input.theta, input.phi)
}

fn y_lm_nonnegative_order(degree: usize, order: usize, theta: f64, phi: f64) -> Complex64 {
    let x = theta.cos();
    let associated_legendre = associated_legendre_polynomial(degree, order, x);
    let normalization =
        (((2 * degree + 1) as f64) * factorial_ratio(degree, order) / (4.0 * PI)).sqrt();
    let phase = Complex64::from_polar(1.0, (order as f64) * phi);

    phase * (normalization * associated_legendre)
}

fn associated_legendre_polynomial(degree: usize, order: usize, x: f64) -> f64 {
    debug_assert!(order <= degree);

    let mut p_mm = 1.0;
    if order > 0 {
        let root = (1.0 - x * x).max(0.0).sqrt();
        for k in 1..=order {
            p_mm *= -((2 * k - 1) as f64) * root;
        }
    }

    if degree == order {
        return p_mm;
    }

    let p_m_plus_1_m = x * ((2 * order + 1) as f64) * p_mm;
    if degree == order + 1 {
        return p_m_plus_1_m;
    }

    let mut p_lm2 = p_mm;
    let mut p_lm1 = p_m_plus_1_m;
    for l in (order + 2)..=degree {
        let numerator = ((2 * l - 1) as f64) * x * p_lm1 - ((l + order - 1) as f64) * p_lm2;
        let p_lm = numerator / ((l - order) as f64);
        p_lm2 = p_lm1;
        p_lm1 = p_lm;
    }

    p_lm1
}

fn factorial_ratio(degree: usize, order: usize) -> f64 {
    if order == 0 {
        return 1.0;
    }

    let mut ratio = 1.0;
    for term in (degree - order + 1)..=(degree + order) {
        ratio /= term as f64;
    }

    ratio
}

#[cfg(test)]
mod tests {
    use super::{SphericalHarmonicsInput, spherical_y, y_lm};
    use crate::common::constants::PI;
    use num_complex::Complex64;

    #[test]
    fn y_lm_matches_representative_known_values() {
        let y00 = y_lm(0, 0, 1.2, -0.8);
        assert_complex_close(
            "Y_0^0",
            Complex64::new((1.0 / (4.0 * PI)).sqrt(), 0.0),
            y00,
            1.0e-14,
            1.0e-13,
        );

        let theta = PI / 3.0;
        let y10 = y_lm(1, 0, theta, 0.4);
        assert_complex_close(
            "Y_1^0",
            Complex64::new((3.0 / (4.0 * PI)).sqrt() * theta.cos(), 0.0),
            y10,
            1.0e-14,
            1.0e-13,
        );

        let y11 = y_lm(1, 1, PI / 2.0, 0.0);
        assert_complex_close(
            "Y_1^1",
            Complex64::new(-(3.0 / (8.0 * PI)).sqrt(), 0.0),
            y11,
            1.0e-14,
            1.0e-13,
        );
    }

    #[test]
    fn y_lm_satisfies_negative_order_symmetry_identity() {
        let theta = 1.1;
        let phi = -0.7;

        for degree in 1..=6 {
            for order in 1..=degree {
                let positive = y_lm(degree, order, theta, phi);
                let expected_negative = if order % 2 == 0 {
                    positive.conj()
                } else {
                    -positive.conj()
                };
                let actual_negative = y_lm(degree, -order, theta, phi);

                assert_complex_close(
                    &format!("l={degree} m={order}"),
                    expected_negative,
                    actual_negative,
                    1.0e-13,
                    1.0e-12,
                );
            }
        }
    }

    #[test]
    fn y_lm_satisfies_normalization_sum_rule() {
        let samples = [(0.3, -1.2), (1.1, 0.4), (2.4, 2.2)];

        for degree in [0, 1, 2, 4, 6] {
            let expected_power = (2 * degree + 1) as f64 / (4.0 * PI);
            for (theta, phi) in samples {
                let mut accumulated = 0.0;
                for order in -degree..=degree {
                    accumulated += y_lm(degree, order, theta, phi).norm_sqr();
                }

                assert_scalar_close(
                    &format!("l={degree} theta={theta} phi={phi}"),
                    expected_power,
                    accumulated,
                    5.0e-12,
                    5.0e-11,
                );
            }
        }
    }

    #[test]
    fn spherical_y_forwards_struct_input_to_y_lm() {
        let input = SphericalHarmonicsInput::new(3, -2, 0.8, 1.4);
        let expected = y_lm(input.degree, input.order, input.theta, input.phi);
        let actual = spherical_y(input);

        assert_complex_close("spherical_y", expected, actual, 1.0e-15, 1.0e-15);
    }

    fn assert_scalar_close(label: &str, expected: f64, actual: f64, abs_tol: f64, rel_tol: f64) {
        let abs_diff = (actual - expected).abs();
        let rel_diff = abs_diff / expected.abs().max(1.0);
        assert!(
            abs_diff <= abs_tol || rel_diff <= rel_tol,
            "{label} expected={expected:.15e} actual={actual:.15e} abs_diff={abs_diff:.15e} rel_diff={rel_diff:.15e} abs_tol={abs_tol:.15e} rel_tol={rel_tol:.15e}"
        );
    }

    fn assert_complex_close(
        label: &str,
        expected: Complex64,
        actual: Complex64,
        abs_tol: f64,
        rel_tol: f64,
    ) {
        let abs_diff = (actual - expected).norm();
        let rel_diff = abs_diff / expected.norm().max(1.0);
        assert!(
            abs_diff <= abs_tol || rel_diff <= rel_tol,
            "{label} expected=({:.15e},{:.15e}) actual=({:.15e},{:.15e}) abs_diff={:.15e} rel_diff={:.15e} abs_tol={:.15e} rel_tol={:.15e}",
            expected.re,
            expected.im,
            actual.re,
            actual.im,
            abs_diff,
            rel_diff,
            abs_tol,
            rel_tol
        );
    }
}
