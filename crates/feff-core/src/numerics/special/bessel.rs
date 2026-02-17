use num_complex::Complex64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphericalBesselInput {
    pub order: usize,
    pub argument: Complex64,
}

impl SphericalBesselInput {
    pub fn new(order: usize, argument: Complex64) -> Self {
        Self { order, argument }
    }
}

pub trait SphericalBesselApi {
    fn spherical_j(&self, input: SphericalBesselInput) -> Complex64;
    fn spherical_n(&self, input: SphericalBesselInput) -> Complex64;
    fn spherical_h1(&self, input: SphericalBesselInput) -> Complex64;
}
