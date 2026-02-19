use num_complex::Complex64;

const TWO: f64 = 2.0;
const FOUR: f64 = 4.0;
const EIGHT: f64 = 8.0;
const TWELVE: f64 = 12.0;
const TWENTY_SEVEN: f64 = 27.0;
const SEVENTY_TWO: f64 = 72.0;
const TWO_TO_ONE_THIRD: f64 = 1.259_921_049_894_873;
const TWO_TO_TWO_THIRDS: f64 = 1.587_401_051_968_199;
const ROOT6: f64 = 2.449_489_742_783_178;

pub fn quartic_in_place(q: &mut [Complex64; 4]) {
    let a = q[0];
    let b = q[1];
    let c = q[2];
    let d = q[3];

    let f = b.powi(2) + TWELVE * a * d;
    let g = TWO * b.powi(3) + TWENTY_SEVEN * a * c.powi(2) - SEVENTY_TWO * a * b * d;

    let a1 = (g + (-FOUR * f.powi(3) + g.powi(2)).sqrt()).powf(1.0 / 3.0);
    let b1 = TWO * TWO_TO_ONE_THIRD * f;

    let p = ((-FOUR * b + b1 / a1 + TWO_TO_TWO_THIRDS * a1) / a).sqrt();

    let d1 = EIGHT * b + b1 / a1 + TWO_TO_TWO_THIRDS * a1;
    let d2 = TWELVE * ROOT6 * c / p;

    let q_plus = (-(d1 + d2) / a).sqrt();
    let q_min = (-(d1 - d2) / a).sqrt();

    let amp = 1.0 / (TWO * ROOT6);

    q[0] = amp * (p - q_plus);
    q[1] = amp * (p + q_plus);
    q[2] = -amp * (p + q_min);
    q[3] = amp * (-p + q_min);
}

pub fn quartic_roots(a: Complex64, b: Complex64, c: Complex64, d: Complex64) -> [Complex64; 4] {
    let mut roots = [a, b, c, d];
    quartic_in_place(&mut roots);
    roots
}

#[cfg(test)]
mod tests {
    use super::{quartic_in_place, quartic_roots};
    use num_complex::Complex64;

    fn evaluate_polynomial(
        a: Complex64,
        b: Complex64,
        c: Complex64,
        d: Complex64,
        x: Complex64,
    ) -> Complex64 {
        a * x.powu(4) + b * x.powu(2) + c * x + d
    }

    #[test]
    fn roots_satisfy_quartic_with_real_known_roots() {
        let a = Complex64::new(1.0, 0.0);
        let b = Complex64::new(-5.0, 0.0);
        let c = Complex64::new(0.0, 0.0);
        let d = Complex64::new(4.0, 0.0);

        let mut roots = quartic_roots(a, b, c, d);
        roots.sort_by(|lhs, rhs| lhs.re.total_cmp(&rhs.re));

        let expected = [-2.0, -1.0, 1.0, 2.0];
        for (root, expected_real) in roots.iter().zip(expected) {
            assert!((root.re - expected_real).abs() <= 1.0e-9);
            assert!(root.im.abs() <= 1.0e-9);
            let residual = evaluate_polynomial(a, b, c, d, *root);
            assert!(residual.norm() <= 1.0e-8, "residual={residual}");
        }
    }

    #[test]
    fn in_place_api_overwrites_coefficients_with_roots() {
        let a = Complex64::new(1.0, 0.0);
        let b = Complex64::new(-5.0, 0.0);
        let c = Complex64::new(0.0, 0.0);
        let d = Complex64::new(4.0, 0.0);

        let mut values = [a, b, c, d];
        quartic_in_place(&mut values);

        assert_ne!(values, [a, b, c, d]);
        for root in values {
            let residual = evaluate_polynomial(a, b, c, d, root);
            assert!(residual.norm() <= 1.0e-8, "residual={residual}");
        }
    }
}
