pub mod bessel;
pub mod harmonics;
pub mod linalg;
pub mod wigner;

pub use bessel::{
    spherical_h, spherical_h1, spherical_j, spherical_n, SphericalBesselApi, SphericalBesselInput,
};
pub use harmonics::{spherical_y, y_lm, SphericalHarmonicsApi, SphericalHarmonicsInput};
pub use linalg::{lu_factorize, lu_solve, LuDecomposition, LuError, LuLinearSolveApi};
pub use wigner::{wigner_3j, wigner_6j, Wigner3jInput, Wigner6jInput, WignerSymbolsApi};

use faer::Mat;
use num_complex::Complex64;

pub type DenseComplexMatrix = Mat<Complex64>;
