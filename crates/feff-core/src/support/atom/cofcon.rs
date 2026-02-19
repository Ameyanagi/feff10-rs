#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CofconResult {
    pub a: f64,
    pub b: f64,
    pub q: f64,
}

pub fn cofcon(mut b: f64, p: f64, q: f64) -> CofconResult {
    let pq = p * q;
    if pq < 0.0 {
        if b >= 0.2 {
            b -= 0.1;
        }
    } else if pq > 0.0 && b <= 0.8 {
        b += 0.1;
    }

    CofconResult {
        a: 1.0 - b,
        b,
        q: p,
    }
}

#[cfg(test)]
mod tests {
    use super::cofcon;

    #[test]
    fn cofcon_increases_b_on_same_sign_errors() {
        let result = cofcon(0.5, 2.0, 1.0);
        assert!((result.b - 0.6).abs() <= 1.0e-12);
        assert!((result.a - 0.4).abs() <= 1.0e-12);
        assert_eq!(result.q, 2.0);
    }

    #[test]
    fn cofcon_decreases_b_on_sign_flip() {
        let result = cofcon(0.5, -1.0, 1.0);
        assert!((result.b - 0.4).abs() <= 1.0e-12);
        assert!((result.a - 0.6).abs() <= 1.0e-12);
        assert_eq!(result.q, -1.0);
    }

    #[test]
    fn cofcon_clamps_adjustment_window_to_fortran_limits() {
        let lower = cofcon(0.1, -1.0, 1.0);
        let upper = cofcon(0.9, 1.0, 1.0);
        assert!((lower.b - 0.1).abs() <= 1.0e-12);
        assert!((upper.b - 0.9).abs() <= 1.0e-12);
    }
}
