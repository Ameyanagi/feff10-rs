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
