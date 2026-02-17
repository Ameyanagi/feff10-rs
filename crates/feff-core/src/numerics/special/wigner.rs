#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wigner3jInput {
    pub two_j1: i32,
    pub two_j2: i32,
    pub two_j3: i32,
    pub two_m1: i32,
    pub two_m2: i32,
    pub two_m3: i32,
}

impl Wigner3jInput {
    pub fn new(
        two_j1: i32,
        two_j2: i32,
        two_j3: i32,
        two_m1: i32,
        two_m2: i32,
        two_m3: i32,
    ) -> Self {
        Self {
            two_j1,
            two_j2,
            two_j3,
            two_m1,
            two_m2,
            two_m3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wigner6jInput {
    pub two_j1: i32,
    pub two_j2: i32,
    pub two_j3: i32,
    pub two_j4: i32,
    pub two_j5: i32,
    pub two_j6: i32,
}

impl Wigner6jInput {
    pub fn new(
        two_j1: i32,
        two_j2: i32,
        two_j3: i32,
        two_j4: i32,
        two_j5: i32,
        two_j6: i32,
    ) -> Self {
        Self {
            two_j1,
            two_j2,
            two_j3,
            two_j4,
            two_j5,
            two_j6,
        }
    }
}

pub trait WignerSymbolsApi {
    fn wigner_3j(&self, input: Wigner3jInput) -> f64;
    fn wigner_6j(&self, input: Wigner6jInput) -> f64;
}
