pub mod bessel;
pub mod convolution;
pub mod harmonics;
pub mod integration;
pub mod linalg;
pub mod wigner;

pub use bessel::{
    spherical_h, spherical_h1, spherical_j, spherical_n, SphericalBesselApi, SphericalBesselInput,
};
pub use convolution::{
    convolve_lorentzian, interpolate_spectrum_linear, ConvolutionError, LorentzianConvolutionInput,
    SpectralConvolutionApi, SpectralInterpolationInput,
};
pub use harmonics::{spherical_y, y_lm, SphericalHarmonicsApi, SphericalHarmonicsInput};
pub use integration::{integrate_somm, RadialIntegrationApi, SommError, SommInput};
pub use linalg::{
    eigen_decompose, eigenvalues, lu_factorize, lu_invert, lu_solve, EigenDecomposition,
    EigenError, EigenvalueSolveApi, LuDecomposition, LuError, LuLinearSolveApi,
};
pub use wigner::{wigner_3j, wigner_6j, Wigner3jInput, Wigner6jInput, WignerSymbolsApi};

use faer::Mat;
use num_complex::Complex64;

pub type DenseComplexMatrix = Mat<Complex64>;
