use num_complex::Complex64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialExtent {
    pub mean: f64,
    pub rms: f64,
    pub max: f64,
}

impl RadialExtent {
    pub fn new(mean: f64, rms: f64, max: f64) -> Self {
        let max = sanitize_positive(max).max(1.0e-6);
        let mean = sanitize_positive(mean).min(max);
        let rms = sanitize_positive(rms).max(mean).max(1.0e-6);
        Self { mean, rms, max }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadialGrid {
    points: Vec<f64>,
    extent: RadialExtent,
    log_step: f64,
}

impl RadialGrid {
    pub fn from_sampled_radii(
        sampled_radii: &[f64],
        point_count: usize,
        log_step_hint: f64,
    ) -> Self {
        let mut sample_count = 0_usize;
        let mut radius_sum = 0.0_f64;
        let mut radius_sq_sum = 0.0_f64;
        let mut radius_max = 0.0_f64;

        for radius in sampled_radii {
            if !radius.is_finite() || *radius < 0.0 {
                continue;
            }

            radius_sum += *radius;
            radius_sq_sum += radius * radius;
            radius_max = radius_max.max(*radius);
            sample_count += 1;
        }

        let extent = if sample_count == 0 {
            RadialExtent::new(1.0, 1.0, 1.0)
        } else {
            let count = sample_count as f64;
            RadialExtent::new(
                radius_sum / count,
                (radius_sq_sum / count).sqrt(),
                radius_max,
            )
        };

        Self::from_extent(extent, point_count, log_step_hint)
    }

    pub fn from_extent(extent: RadialExtent, point_count: usize, log_step_hint: f64) -> Self {
        let point_count = point_count.max(2);
        let radius_min = (extent.mean.max(1.0e-4) * 1.0e-3).max(1.0e-6);
        let radius_max = extent.max.max(radius_min * 1.01);
        let computed_log_step = (radius_max / radius_min).ln() / (point_count - 1) as f64;

        let log_step = if log_step_hint.is_finite() && log_step_hint > 0.0 {
            log_step_hint.max(1.0e-6)
        } else {
            computed_log_step.max(1.0e-6)
        };

        let mut points = Vec::with_capacity(point_count);
        for index in 0..point_count {
            let exponent = (index as f64 * log_step).clamp(-700.0, 700.0);
            let mut radius = radius_min * exponent.exp();
            if !radius.is_finite() || radius <= 0.0 {
                radius = points
                    .last()
                    .copied()
                    .unwrap_or(radius_min)
                    .mul_add(1.25, 0.0);
            }
            points.push(radius);
        }

        if let Some(last) = points.last().copied() {
            if last.is_finite() && last > 0.0 {
                let scale = radius_max / last;
                for point in &mut points {
                    *point *= scale;
                }
            }
        }

        if let Some(first) = points.first_mut() {
            *first = radius_min;
        }
        if let Some(last) = points.last_mut() {
            *last = radius_max;
        }

        for index in 1..points.len() {
            if points[index] <= points[index - 1] {
                points[index] = points[index - 1] * 1.000_001;
            }
        }

        Self {
            points,
            extent,
            log_step: log_step.max(1.0e-6),
        }
    }

    pub fn points(&self) -> &[f64] {
        &self.points
    }

    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    pub fn log_step(&self) -> f64 {
        self.log_step
    }

    pub fn extent(&self) -> RadialExtent {
        self.extent
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoundStateSolverState {
    radial_grid: RadialGrid,
    iteration_limit: usize,
    mixing_parameter: f64,
    muffin_tin_radius: f64,
}

impl BoundStateSolverState {
    pub fn new(
        radial_grid: RadialGrid,
        iteration_limit: usize,
        mixing_parameter: f64,
        muffin_tin_radius: f64,
    ) -> Self {
        let default_radius = radial_grid.extent().max.max(1.0e-6);
        let muffin_tin_radius = if muffin_tin_radius.is_finite() && muffin_tin_radius > 0.0 {
            muffin_tin_radius
        } else {
            default_radius
        };

        Self {
            radial_grid,
            iteration_limit: iteration_limit.max(1),
            mixing_parameter: mixing_parameter.abs().clamp(0.0, 1.0),
            muffin_tin_radius,
        }
    }

    pub fn radial_grid(&self) -> &RadialGrid {
        &self.radial_grid
    }

    pub fn iteration_limit(&self) -> usize {
        self.iteration_limit
    }

    pub fn mixing_parameter(&self) -> f64 {
        self.mixing_parameter
    }

    pub fn muffin_tin_radius(&self) -> f64 {
        self.muffin_tin_radius
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComplexEnergySolverState {
    radial_grid: RadialGrid,
    energy: Complex64,
    max_wave_number: f64,
    channel_count_hint: usize,
}

impl ComplexEnergySolverState {
    pub fn new(
        radial_grid: RadialGrid,
        energy: Complex64,
        max_wave_number: f64,
        channel_count_hint: usize,
    ) -> Self {
        let energy = if energy.re.is_finite() && energy.im.is_finite() {
            energy
        } else {
            Complex64::new(0.0, 0.0)
        };

        let max_wave_number = if max_wave_number.is_finite() && max_wave_number > 0.0 {
            max_wave_number
        } else {
            energy.im.abs().max(1.0e-4)
        };

        Self {
            radial_grid,
            energy,
            max_wave_number,
            channel_count_hint: channel_count_hint.max(1),
        }
    }

    pub fn radial_grid(&self) -> &RadialGrid {
        &self.radial_grid
    }

    pub fn energy(&self) -> Complex64 {
        self.energy
    }

    pub fn max_wave_number(&self) -> f64 {
        self.max_wave_number
    }

    pub fn channel_count_hint(&self) -> usize {
        self.channel_count_hint
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AtomRadialOrbitalInput<'a> {
    occupation: f64,
    valence_occupation: f64,
    large_component: &'a [f64],
    small_component: &'a [f64],
}

impl<'a> AtomRadialOrbitalInput<'a> {
    pub fn new(occupation: f64, large_component: &'a [f64], small_component: &'a [f64]) -> Self {
        let occupation = sanitize_positive(occupation);
        Self {
            occupation,
            valence_occupation: occupation,
            large_component,
            small_component,
        }
    }

    pub fn with_valence_occupation(mut self, valence_occupation: f64) -> Self {
        self.valence_occupation = sanitize_positive(valence_occupation).min(self.occupation);
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomChargeDensityUpdate {
    shell_density: Vec<f64>,
    valence_shell_density: Vec<f64>,
    density_4pi: Vec<f64>,
    valence_density_4pi: Vec<f64>,
    total_charge: f64,
    valence_charge: f64,
    target_charge: f64,
    target_valence_charge: f64,
    normalization_scale: f64,
    valence_normalization_scale: f64,
}

impl AtomChargeDensityUpdate {
    pub fn shell_density(&self) -> &[f64] {
        &self.shell_density
    }

    pub fn valence_shell_density(&self) -> &[f64] {
        &self.valence_shell_density
    }

    pub fn density_4pi(&self) -> &[f64] {
        &self.density_4pi
    }

    pub fn valence_density_4pi(&self) -> &[f64] {
        &self.valence_density_4pi
    }

    pub fn total_charge(&self) -> f64 {
        self.total_charge
    }

    pub fn valence_charge(&self) -> f64 {
        self.valence_charge
    }

    pub fn target_charge(&self) -> f64 {
        self.target_charge
    }

    pub fn target_valence_charge(&self) -> f64 {
        self.target_valence_charge
    }

    pub fn normalization_scale(&self) -> f64 {
        self.normalization_scale
    }

    pub fn valence_normalization_scale(&self) -> f64 {
        self.valence_normalization_scale
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MuffinTinPotentialUpdate {
    electron_potential: Vec<f64>,
    total_potential: Vec<f64>,
    muffin_tin_potential: Vec<f64>,
    enclosed_charge: Vec<f64>,
    boundary_index: usize,
    boundary_radius: f64,
    boundary_value: f64,
}

impl MuffinTinPotentialUpdate {
    pub fn electron_potential(&self) -> &[f64] {
        &self.electron_potential
    }

    pub fn total_potential(&self) -> &[f64] {
        &self.total_potential
    }

    pub fn muffin_tin_potential(&self) -> &[f64] {
        &self.muffin_tin_potential
    }

    pub fn enclosed_charge(&self) -> &[f64] {
        &self.enclosed_charge
    }

    pub fn boundary_index(&self) -> usize {
        self.boundary_index
    }

    pub fn boundary_radius(&self) -> f64 {
        self.boundary_radius
    }

    pub fn boundary_value(&self) -> f64 {
        self.boundary_value
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomScfOrbitalSpec {
    principal_quantum_number: usize,
    kappa: i32,
    occupation: f64,
    valence_occupation: f64,
    energy_bounds: Option<(f64, f64)>,
    match_index: Option<usize>,
    convergence_tolerance: Option<f64>,
}

impl AtomScfOrbitalSpec {
    pub fn new(principal_quantum_number: usize, kappa: i32, occupation: f64) -> Self {
        let occupation = sanitize_positive(occupation);
        Self {
            principal_quantum_number,
            kappa,
            occupation,
            valence_occupation: occupation,
            energy_bounds: None,
            match_index: None,
            convergence_tolerance: None,
        }
    }

    pub fn with_valence_occupation(mut self, valence_occupation: f64) -> Self {
        self.valence_occupation = sanitize_positive(valence_occupation).min(self.occupation);
        self
    }

    pub fn with_energy_bounds(mut self, energy_min: f64, energy_max: f64) -> Self {
        self.energy_bounds = Some((energy_min, energy_max));
        self
    }

    pub fn with_match_index(mut self, match_index: usize) -> Self {
        self.match_index = Some(match_index);
        self
    }

    pub fn with_convergence_tolerance(mut self, convergence_tolerance: f64) -> Self {
        self.convergence_tolerance = Some(convergence_tolerance);
        self
    }

    pub fn principal_quantum_number(&self) -> usize {
        self.principal_quantum_number
    }

    pub fn kappa(&self) -> i32 {
        self.kappa
    }

    pub fn occupation(&self) -> f64 {
        self.occupation
    }

    pub fn valence_occupation(&self) -> f64 {
        self.valence_occupation
    }

    pub fn energy_bounds(&self) -> Option<(f64, f64)> {
        self.energy_bounds
    }

    pub fn match_index(&self) -> Option<usize> {
        self.match_index
    }

    pub fn convergence_tolerance(&self) -> Option<f64> {
        self.convergence_tolerance
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomScfInput<'a> {
    state: &'a BoundStateSolverState,
    initial_potential: &'a [f64],
    orbitals: &'a [AtomScfOrbitalSpec],
    nuclear_charge: f64,
    muffin_tin_radius: Option<f64>,
    max_iterations: usize,
    potential_tolerance: f64,
    charge_tolerance: f64,
    broyden_history: usize,
    broyden_regularization: f64,
}

impl<'a> AtomScfInput<'a> {
    pub fn new(
        state: &'a BoundStateSolverState,
        initial_potential: &'a [f64],
        orbitals: &'a [AtomScfOrbitalSpec],
        nuclear_charge: f64,
    ) -> Self {
        Self {
            state,
            initial_potential,
            orbitals,
            nuclear_charge,
            muffin_tin_radius: None,
            max_iterations: state.iteration_limit(),
            potential_tolerance: 1.0e-6,
            charge_tolerance: 1.0e-6,
            broyden_history: 8,
            broyden_regularization: 1.0e-14,
        }
    }

    pub fn with_muffin_tin_radius(mut self, muffin_tin_radius: f64) -> Self {
        self.muffin_tin_radius = Some(muffin_tin_radius);
        self
    }

    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations.max(1);
        self
    }

    pub fn with_potential_tolerance(mut self, potential_tolerance: f64) -> Self {
        self.potential_tolerance = potential_tolerance;
        self
    }

    pub fn with_charge_tolerance(mut self, charge_tolerance: f64) -> Self {
        self.charge_tolerance = charge_tolerance;
        self
    }

    pub fn with_broyden_history(mut self, broyden_history: usize) -> Self {
        self.broyden_history = broyden_history.max(1);
        self
    }

    pub fn with_broyden_regularization(mut self, broyden_regularization: f64) -> Self {
        self.broyden_regularization = broyden_regularization;
        self
    }

    pub fn state(&self) -> &BoundStateSolverState {
        self.state
    }

    pub fn initial_potential(&self) -> &[f64] {
        self.initial_potential
    }

    pub fn orbitals(&self) -> &[AtomScfOrbitalSpec] {
        self.orbitals
    }

    pub fn nuclear_charge(&self) -> f64 {
        self.nuclear_charge
    }

    pub fn muffin_tin_radius(&self) -> Option<f64> {
        self.muffin_tin_radius
    }

    pub fn max_iterations(&self) -> usize {
        self.max_iterations
    }

    pub fn potential_tolerance(&self) -> f64 {
        self.potential_tolerance
    }

    pub fn charge_tolerance(&self) -> f64 {
        self.charge_tolerance
    }

    pub fn broyden_history(&self) -> usize {
        self.broyden_history
    }

    pub fn broyden_regularization(&self) -> f64 {
        self.broyden_regularization
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomScfIteration {
    iteration: usize,
    residual_rms: f64,
    residual_max: f64,
    charge_delta: f64,
    step_rms: f64,
    converged: bool,
}

impl AtomScfIteration {
    pub fn iteration(&self) -> usize {
        self.iteration
    }

    pub fn residual_rms(&self) -> f64 {
        self.residual_rms
    }

    pub fn residual_max(&self) -> f64 {
        self.residual_max
    }

    pub fn charge_delta(&self) -> f64 {
        self.charge_delta
    }

    pub fn step_rms(&self) -> f64 {
        self.step_rms
    }

    pub fn converged(&self) -> bool {
        self.converged
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomScfResult {
    converged: bool,
    iteration_count: usize,
    iterations: Vec<AtomScfIteration>,
    potential: Vec<f64>,
    shell_density: Vec<f64>,
    valence_shell_density: Vec<f64>,
    total_charge: f64,
    target_charge: f64,
    valence_charge: f64,
    target_valence_charge: f64,
    boundary_index: usize,
    boundary_radius: f64,
    orbitals: Vec<RadialDiracSolution>,
}

impl AtomScfResult {
    pub fn converged(&self) -> bool {
        self.converged
    }

    pub fn iteration_count(&self) -> usize {
        self.iteration_count
    }

    pub fn iterations(&self) -> &[AtomScfIteration] {
        &self.iterations
    }

    pub fn potential(&self) -> &[f64] {
        &self.potential
    }

    pub fn shell_density(&self) -> &[f64] {
        &self.shell_density
    }

    pub fn valence_shell_density(&self) -> &[f64] {
        &self.valence_shell_density
    }

    pub fn total_charge(&self) -> f64 {
        self.total_charge
    }

    pub fn target_charge(&self) -> f64 {
        self.target_charge
    }

    pub fn valence_charge(&self) -> f64 {
        self.valence_charge
    }

    pub fn target_valence_charge(&self) -> f64 {
        self.target_valence_charge
    }

    pub fn boundary_index(&self) -> usize {
        self.boundary_index
    }

    pub fn boundary_radius(&self) -> f64 {
        self.boundary_radius
    }

    pub fn orbitals(&self) -> &[RadialDiracSolution] {
        &self.orbitals
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AtomTotalEnergyTerms {
    direct_coulomb: f64,
    exchange_coulomb: f64,
    magnetic: f64,
    retardation: f64,
}

impl AtomTotalEnergyTerms {
    pub fn new(
        direct_coulomb: f64,
        exchange_coulomb: f64,
        magnetic: f64,
        retardation: f64,
    ) -> Self {
        Self {
            direct_coulomb: if direct_coulomb.is_finite() {
                direct_coulomb
            } else {
                0.0
            },
            exchange_coulomb: if exchange_coulomb.is_finite() {
                exchange_coulomb
            } else {
                0.0
            },
            magnetic: if magnetic.is_finite() { magnetic } else { 0.0 },
            retardation: if retardation.is_finite() {
                retardation
            } else {
                0.0
            },
        }
    }

    pub fn direct_coulomb(&self) -> f64 {
        self.direct_coulomb
    }

    pub fn exchange_coulomb(&self) -> f64 {
        self.exchange_coulomb
    }

    pub fn magnetic(&self) -> f64 {
        self.magnetic
    }

    pub fn retardation(&self) -> f64 {
        self.retardation
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomScfOutputs {
    total_energy: f64,
    s02: f64,
    orbital_energy_sum: f64,
    energy_terms: AtomTotalEnergyTerms,
}

impl AtomScfOutputs {
    pub fn total_energy(&self) -> f64 {
        self.total_energy
    }

    pub fn s02(&self) -> f64 {
        self.s02
    }

    pub fn orbital_energy_sum(&self) -> f64 {
        self.orbital_energy_sum
    }

    pub fn energy_terms(&self) -> AtomTotalEnergyTerms {
        self.energy_terms
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomScfOutputInput<'a> {
    state: &'a BoundStateSolverState,
    result: &'a AtomScfResult,
    orbitals: &'a [AtomScfOrbitalSpec],
    nuclear_charge: f64,
    core_hole_orbital: Option<usize>,
    reference_orbitals: Option<&'a [RadialDiracSolution]>,
}

impl<'a> AtomScfOutputInput<'a> {
    pub fn new(
        state: &'a BoundStateSolverState,
        result: &'a AtomScfResult,
        orbitals: &'a [AtomScfOrbitalSpec],
        nuclear_charge: f64,
    ) -> Self {
        Self {
            state,
            result,
            orbitals,
            nuclear_charge,
            core_hole_orbital: None,
            reference_orbitals: None,
        }
    }

    pub fn with_core_hole_orbital(mut self, core_hole_orbital: usize) -> Self {
        self.core_hole_orbital = Some(core_hole_orbital);
        self
    }

    pub fn with_reference_orbitals(
        mut self,
        reference_orbitals: &'a [RadialDiracSolution],
    ) -> Self {
        self.reference_orbitals = Some(reference_orbitals);
        self
    }

    pub fn state(&self) -> &BoundStateSolverState {
        self.state
    }

    pub fn result(&self) -> &AtomScfResult {
        self.result
    }

    pub fn orbitals(&self) -> &[AtomScfOrbitalSpec] {
        self.orbitals
    }

    pub fn nuclear_charge(&self) -> f64 {
        self.nuclear_charge
    }

    pub fn core_hole_orbital(&self) -> Option<usize> {
        self.core_hole_orbital
    }

    pub fn reference_orbitals(&self) -> Option<&[RadialDiracSolution]> {
        self.reference_orbitals
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AtomS02Input<'a> {
    kappa: &'a [i32],
    core_occupations: &'a [f64],
    overlap_matrix: &'a [f64],
    core_hole_orbital: Option<usize>,
}

impl<'a> AtomS02Input<'a> {
    pub fn new(kappa: &'a [i32], core_occupations: &'a [f64], overlap_matrix: &'a [f64]) -> Self {
        Self {
            kappa,
            core_occupations,
            overlap_matrix,
            core_hole_orbital: None,
        }
    }

    pub fn with_core_hole_orbital(mut self, core_hole_orbital: usize) -> Self {
        self.core_hole_orbital = Some(core_hole_orbital);
        self
    }

    pub fn kappa(&self) -> &[i32] {
        self.kappa
    }

    pub fn core_occupations(&self) -> &[f64] {
        self.core_occupations
    }

    pub fn overlap_matrix(&self) -> &[f64] {
        self.overlap_matrix
    }

    pub fn core_hole_orbital(&self) -> Option<usize> {
        self.core_hole_orbital
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AtomScfKernelError {
    #[error("bound-state radial grid requires at least 2 points, got {actual}")]
    InsufficientGridPoints { actual: usize },
    #[error("charge-density update requires at least one orbital input")]
    NoOrbitals,
    #[error(
        "orbital {orbital_index} component length mismatch: expected {expected}, large {large_actual}, small {small_actual}"
    )]
    OrbitalLengthMismatch {
        orbital_index: usize,
        expected: usize,
        large_actual: usize,
        small_actual: usize,
    },
    #[error("previous shell density length mismatch: expected {expected}, got {actual}")]
    PreviousShellDensityLengthMismatch { expected: usize, actual: usize },
    #[error("shell density length mismatch: expected {expected}, got {actual}")]
    ShellDensityLengthMismatch { expected: usize, actual: usize },
    #[error("initial potential length mismatch: expected {expected}, got {actual}")]
    InitialPotentialLengthMismatch { expected: usize, actual: usize },
    #[error("invalid nuclear charge: {value}")]
    InvalidNuclearCharge { value: f64 },
    #[error("invalid SCF tolerance for {field}: {value}")]
    InvalidScfTolerance { field: &'static str, value: f64 },
    #[error("SCF loop requires at least one orbital specification")]
    NoScfOrbitals,
    #[error("Dirac solve failed for orbital index {orbital_index}: {source}")]
    DiracSolveFailure {
        orbital_index: usize,
        #[source]
        source: RadialDiracError,
    },
    #[error(
        "SCF loop did not converge after {iterations} iterations (residual_rms={last_residual_rms}, charge_delta={last_charge_delta})"
    )]
    ScfNoConvergence {
        iterations: usize,
        last_residual_rms: f64,
        last_charge_delta: f64,
    },
    #[error(
        "atom total-energy input length mismatch for {field}: expected {expected}, got {actual}"
    )]
    TotalEnergyLengthMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("atom total-energy estimate requires finite positive nuclear charge, got {value}")]
    InvalidEnergyNuclearCharge { value: f64 },
    #[error(
        "S02 overlap matrix shape mismatch: expected square matrix with {expected} entries, got {actual}"
    )]
    S02OverlapMatrixShapeMismatch { expected: usize, actual: usize },
    #[error("S02 core-hole orbital index {index} out of range for {orbital_count} orbitals")]
    S02InvalidCoreHoleOrbital { index: usize, orbital_count: usize },
    #[error("S02 kappa subshell {kappa} has {count} orbitals (max supported {max})")]
    S02SubshellOverflow {
        kappa: i32,
        count: usize,
        max: usize,
    },
    #[error("charge-density integral is non-positive for target charge {target_charge}")]
    NonPositiveChargeIntegral { target_charge: f64 },
}

pub fn update_atom_charge_density(
    state: &BoundStateSolverState,
    orbitals: &[AtomRadialOrbitalInput<'_>],
    previous_shell_density: Option<&[f64]>,
    mixing: f64,
) -> Result<AtomChargeDensityUpdate, AtomScfKernelError> {
    let radial_grid = state.radial_grid().points();
    if radial_grid.len() < 2 {
        return Err(AtomScfKernelError::InsufficientGridPoints {
            actual: radial_grid.len(),
        });
    }
    if orbitals.is_empty() {
        return Err(AtomScfKernelError::NoOrbitals);
    }
    if let Some(previous_shell_density) = previous_shell_density {
        if previous_shell_density.len() != radial_grid.len() {
            return Err(AtomScfKernelError::PreviousShellDensityLengthMismatch {
                expected: radial_grid.len(),
                actual: previous_shell_density.len(),
            });
        }
    }

    let mut shell_density = vec![0.0_f64; radial_grid.len()];
    let mut valence_shell_density = vec![0.0_f64; radial_grid.len()];
    let mut target_charge = 0.0_f64;
    let mut target_valence_charge = 0.0_f64;

    for (orbital_index, orbital) in orbitals.iter().enumerate() {
        if orbital.large_component.len() != radial_grid.len()
            || orbital.small_component.len() != radial_grid.len()
        {
            return Err(AtomScfKernelError::OrbitalLengthMismatch {
                orbital_index,
                expected: radial_grid.len(),
                large_actual: orbital.large_component.len(),
                small_actual: orbital.small_component.len(),
            });
        }

        let occupation = sanitize_positive(orbital.occupation);
        let valence_occupation = sanitize_positive(orbital.valence_occupation).min(occupation);
        target_charge += occupation;
        target_valence_charge += valence_occupation;

        for index in 0..radial_grid.len() {
            let large = orbital.large_component[index];
            let small = orbital.small_component[index];
            let amplitude = if large.is_finite() && small.is_finite() {
                (large * large + small * small).max(0.0)
            } else {
                0.0
            };
            shell_density[index] += occupation * amplitude;
            valence_shell_density[index] += valence_occupation * amplitude;
        }
    }

    if let Some(previous_shell_density) = previous_shell_density {
        let mixed_weight = mixing.abs().clamp(0.0, 1.0);
        let previous_weight = 1.0 - mixed_weight;
        for (index, current) in shell_density.iter_mut().enumerate() {
            let previous = sanitize_positive(previous_shell_density[index]);
            *current = previous_weight.mul_add(previous, mixed_weight * *current);
        }
    }

    for value in &mut shell_density {
        *value = sanitize_positive(*value);
    }
    for value in &mut valence_shell_density {
        *value = sanitize_positive(*value);
    }

    let mut normalization_scale = 0.0_f64;
    if target_charge > 0.0 {
        let integrated_charge = integrate_shell_density(radial_grid, &shell_density);
        if integrated_charge <= 0.0 {
            return Err(AtomScfKernelError::NonPositiveChargeIntegral { target_charge });
        }
        normalization_scale = target_charge / integrated_charge;
        for value in &mut shell_density {
            *value *= normalization_scale;
        }
    }

    let mut valence_normalization_scale = 0.0_f64;
    if target_valence_charge > 0.0 {
        let integrated_valence_charge =
            integrate_shell_density(radial_grid, &valence_shell_density);
        if integrated_valence_charge <= 0.0 {
            return Err(AtomScfKernelError::NonPositiveChargeIntegral {
                target_charge: target_valence_charge,
            });
        }
        valence_normalization_scale = target_valence_charge / integrated_valence_charge;
        for value in &mut valence_shell_density {
            *value *= valence_normalization_scale;
        }
    }

    let total_charge = integrate_shell_density(radial_grid, &shell_density);
    let valence_charge = integrate_shell_density(radial_grid, &valence_shell_density);
    let density_4pi = shell_to_density_4pi(radial_grid, &shell_density);
    let valence_density_4pi = shell_to_density_4pi(radial_grid, &valence_shell_density);

    Ok(AtomChargeDensityUpdate {
        shell_density,
        valence_shell_density,
        density_4pi,
        valence_density_4pi,
        total_charge,
        valence_charge,
        target_charge,
        target_valence_charge,
        normalization_scale,
        valence_normalization_scale,
    })
}

pub fn update_muffin_tin_potential(
    state: &BoundStateSolverState,
    shell_density: &[f64],
    nuclear_charge: f64,
    muffin_tin_radius: Option<f64>,
) -> Result<MuffinTinPotentialUpdate, AtomScfKernelError> {
    let radial_grid = state.radial_grid().points();
    if radial_grid.len() < 2 {
        return Err(AtomScfKernelError::InsufficientGridPoints {
            actual: radial_grid.len(),
        });
    }
    if shell_density.len() != radial_grid.len() {
        return Err(AtomScfKernelError::ShellDensityLengthMismatch {
            expected: radial_grid.len(),
            actual: shell_density.len(),
        });
    }
    if !nuclear_charge.is_finite() || nuclear_charge <= 0.0 {
        return Err(AtomScfKernelError::InvalidNuclearCharge {
            value: nuclear_charge,
        });
    }

    let mut sanitized_shell_density = vec![0.0_f64; shell_density.len()];
    for (target, source) in sanitized_shell_density
        .iter_mut()
        .zip(shell_density.iter().copied())
    {
        *target = sanitize_positive(source);
    }

    let enclosed_charge = cumulative_shell_charge(radial_grid, &sanitized_shell_density);
    let tail_integral = reverse_shell_over_radius_integral(radial_grid, &sanitized_shell_density);
    let mut electron_potential = vec![0.0_f64; radial_grid.len()];
    let mut total_potential = vec![0.0_f64; radial_grid.len()];

    for index in 0..radial_grid.len() {
        let radius = radial_grid[index].max(MIN_RADIUS);
        electron_potential[index] = enclosed_charge[index] / radius + tail_integral[index];
        total_potential[index] = electron_potential[index] - nuclear_charge / radius;
    }

    let boundary_radius_target = muffin_tin_radius
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or_else(|| state.muffin_tin_radius());
    let boundary_index = radial_grid
        .iter()
        .position(|radius| *radius >= boundary_radius_target)
        .unwrap_or(radial_grid.len() - 1);
    let shift = -total_potential[boundary_index];
    let mut muffin_tin_potential = total_potential
        .iter()
        .map(|value| value + shift)
        .collect::<Vec<_>>();
    let boundary_value = muffin_tin_potential[boundary_index];
    for value in muffin_tin_potential.iter_mut().skip(boundary_index + 1) {
        *value = boundary_value;
    }

    Ok(MuffinTinPotentialUpdate {
        electron_potential,
        total_potential,
        muffin_tin_potential,
        enclosed_charge,
        boundary_index,
        boundary_radius: radial_grid[boundary_index],
        boundary_value,
    })
}

pub fn solve_atom_scf(input: AtomScfInput<'_>) -> Result<AtomScfResult, AtomScfKernelError> {
    let radial_grid = input.state().radial_grid().points();
    if radial_grid.len() < 2 {
        return Err(AtomScfKernelError::InsufficientGridPoints {
            actual: radial_grid.len(),
        });
    }
    if input.initial_potential().len() != radial_grid.len() {
        return Err(AtomScfKernelError::InitialPotentialLengthMismatch {
            expected: radial_grid.len(),
            actual: input.initial_potential().len(),
        });
    }
    if input.orbitals().is_empty() {
        return Err(AtomScfKernelError::NoScfOrbitals);
    }
    if !input.nuclear_charge().is_finite() || input.nuclear_charge() <= 0.0 {
        return Err(AtomScfKernelError::InvalidNuclearCharge {
            value: input.nuclear_charge(),
        });
    }
    if !input.potential_tolerance().is_finite() || input.potential_tolerance() <= 0.0 {
        return Err(AtomScfKernelError::InvalidScfTolerance {
            field: "potential_tolerance",
            value: input.potential_tolerance(),
        });
    }
    if !input.charge_tolerance().is_finite() || input.charge_tolerance() <= 0.0 {
        return Err(AtomScfKernelError::InvalidScfTolerance {
            field: "charge_tolerance",
            value: input.charge_tolerance(),
        });
    }
    if !input.broyden_regularization().is_finite() || input.broyden_regularization() < 0.0 {
        return Err(AtomScfKernelError::InvalidScfTolerance {
            field: "broyden_regularization",
            value: input.broyden_regularization(),
        });
    }

    let max_iterations = input.max_iterations().max(1);
    let mut potential = sanitize_potential(input.initial_potential());
    let mut iterations = Vec::with_capacity(max_iterations);
    let mut previous_residual: Option<Vec<f64>> = None;
    let mut previous_step: Option<Vec<f64>> = None;
    let mut broyden = BroydenMixer::new(
        input.state().mixing_parameter().clamp(1.0e-6, 1.0),
        input.broyden_history().max(1),
        input.broyden_regularization(),
    );
    let mut last_residual_rms = f64::INFINITY;
    let mut last_charge_delta = f64::INFINITY;

    for iteration_index in 0..max_iterations {
        let solved_orbitals = solve_atom_scf_orbitals(
            input.state(),
            &potential,
            input.orbitals(),
            input.nuclear_charge(),
        )?;

        let mut orbital_inputs = Vec::with_capacity(solved_orbitals.len());
        for (orbital, solution) in input.orbitals().iter().zip(solved_orbitals.iter()) {
            orbital_inputs.push(
                AtomRadialOrbitalInput::new(
                    orbital.occupation(),
                    solution.large_component(),
                    solution.small_component(),
                )
                .with_valence_occupation(orbital.valence_occupation()),
            );
        }

        let charge_update =
            update_atom_charge_density(input.state(), &orbital_inputs, None, 1.0_f64)?;
        let potential_update = update_muffin_tin_potential(
            input.state(),
            charge_update.shell_density(),
            input.nuclear_charge(),
            input.muffin_tin_radius(),
        )?;
        let candidate_potential = potential_update.muffin_tin_potential().to_vec();
        let residual = subtract_vectors(&candidate_potential, &potential);
        let residual_rms = vector_rms(&residual);
        let residual_max = vector_max_abs(&residual);
        let charge_delta = (charge_update.total_charge() - charge_update.target_charge()).abs();
        last_residual_rms = residual_rms;
        last_charge_delta = charge_delta;

        let iteration = iteration_index + 1;
        let converged =
            residual_rms <= input.potential_tolerance() && charge_delta <= input.charge_tolerance();
        if converged {
            iterations.push(AtomScfIteration {
                iteration,
                residual_rms,
                residual_max,
                charge_delta,
                step_rms: 0.0,
                converged: true,
            });
            return Ok(AtomScfResult {
                converged: true,
                iteration_count: iteration,
                iterations,
                potential: candidate_potential,
                shell_density: charge_update.shell_density().to_vec(),
                valence_shell_density: charge_update.valence_shell_density().to_vec(),
                total_charge: charge_update.total_charge(),
                target_charge: charge_update.target_charge(),
                valence_charge: charge_update.valence_charge(),
                target_valence_charge: charge_update.target_valence_charge(),
                boundary_index: potential_update.boundary_index(),
                boundary_radius: potential_update.boundary_radius(),
                orbitals: solved_orbitals,
            });
        }

        if let (Some(previous_residual), Some(previous_step)) =
            (previous_residual.as_ref(), previous_step.as_ref())
        {
            let delta_residual = subtract_vectors(&residual, previous_residual);
            broyden.push_update(previous_step, &delta_residual);
        }

        let step = broyden.mix_step(&residual);
        let step_rms = vector_rms(&step);
        for (value, delta) in potential.iter_mut().zip(step.iter().copied()) {
            *value += delta;
        }

        iterations.push(AtomScfIteration {
            iteration,
            residual_rms,
            residual_max,
            charge_delta,
            step_rms,
            converged: false,
        });

        previous_residual = Some(residual);
        previous_step = Some(step);
    }

    Err(AtomScfKernelError::ScfNoConvergence {
        iterations: max_iterations,
        last_residual_rms,
        last_charge_delta,
    })
}

pub fn compute_atom_scf_outputs(
    input: AtomScfOutputInput<'_>,
) -> Result<AtomScfOutputs, AtomScfKernelError> {
    let orbital_count = input.orbitals().len();
    if input.result().orbitals().len() != orbital_count {
        return Err(AtomScfKernelError::TotalEnergyLengthMismatch {
            field: "result.orbitals",
            expected: orbital_count,
            actual: input.result().orbitals().len(),
        });
    }
    if let Some(reference_orbitals) = input.reference_orbitals() {
        if reference_orbitals.len() != orbital_count {
            return Err(AtomScfKernelError::TotalEnergyLengthMismatch {
                field: "reference_orbitals",
                expected: orbital_count,
                actual: reference_orbitals.len(),
            });
        }
    }

    let terms = estimate_atom_total_energy_terms(
        input.state(),
        input.result(),
        input.orbitals(),
        input.nuclear_charge(),
    )?;
    let orbital_energies = input
        .result()
        .orbitals()
        .iter()
        .map(|orbital| orbital.energy())
        .collect::<Vec<_>>();
    let occupations = input
        .orbitals()
        .iter()
        .map(|orbital| sanitize_positive(orbital.occupation()))
        .collect::<Vec<_>>();
    let orbital_energy_sum = orbital_energy_sum(&orbital_energies, &occupations)?;
    let total_energy = atom_total_energy_from_terms(&orbital_energies, &occupations, terms)?;

    let overlap_matrix = build_orbital_overlap_matrix(
        input.state().radial_grid().points(),
        input.result().orbitals(),
        input
            .reference_orbitals()
            .unwrap_or(input.result().orbitals()),
    )?;
    let kappa = input
        .orbitals()
        .iter()
        .map(|orbital| orbital.kappa())
        .collect::<Vec<_>>();
    let core_occupations = input
        .orbitals()
        .iter()
        .map(|orbital| {
            sanitize_positive(orbital.occupation())
                - sanitize_positive(orbital.valence_occupation())
        })
        .collect::<Vec<_>>();

    let mut s02_input = AtomS02Input::new(&kappa, &core_occupations, &overlap_matrix);
    if let Some(core_hole_orbital) = input.core_hole_orbital() {
        s02_input = s02_input.with_core_hole_orbital(core_hole_orbital);
    }
    let s02 = atom_s02_from_overlap(s02_input)?;

    Ok(AtomScfOutputs {
        total_energy,
        s02,
        orbital_energy_sum,
        energy_terms: terms,
    })
}

pub fn estimate_atom_total_energy_terms(
    state: &BoundStateSolverState,
    result: &AtomScfResult,
    orbitals: &[AtomScfOrbitalSpec],
    nuclear_charge: f64,
) -> Result<AtomTotalEnergyTerms, AtomScfKernelError> {
    if !nuclear_charge.is_finite() || nuclear_charge <= 0.0 {
        return Err(AtomScfKernelError::InvalidEnergyNuclearCharge {
            value: nuclear_charge,
        });
    }

    let radial_grid = state.radial_grid().points();
    let point_count = radial_grid.len();
    if result.shell_density().len() != point_count {
        return Err(AtomScfKernelError::TotalEnergyLengthMismatch {
            field: "result.shell_density",
            expected: point_count,
            actual: result.shell_density().len(),
        });
    }
    if result.valence_shell_density().len() != point_count {
        return Err(AtomScfKernelError::TotalEnergyLengthMismatch {
            field: "result.valence_shell_density",
            expected: point_count,
            actual: result.valence_shell_density().len(),
        });
    }
    if result.potential().len() != point_count {
        return Err(AtomScfKernelError::TotalEnergyLengthMismatch {
            field: "result.potential",
            expected: point_count,
            actual: result.potential().len(),
        });
    }
    if result.orbitals().len() != orbitals.len() {
        return Err(AtomScfKernelError::TotalEnergyLengthMismatch {
            field: "orbitals",
            expected: result.orbitals().len(),
            actual: orbitals.len(),
        });
    }

    let mut direct_coulomb = 0.0_f64;
    let mut exchange_coulomb = 0.0_f64;
    let mut magnetic = 0.0_f64;

    for index in 1..point_count {
        let step = (radial_grid[index] - radial_grid[index - 1]).max(0.0);
        if step <= 0.0 {
            continue;
        }

        let left_radius = radial_grid[index - 1].max(MIN_RADIUS);
        let right_radius = radial_grid[index].max(MIN_RADIUS);
        let left_screening = (result.potential()[index - 1] + nuclear_charge / left_radius).abs();
        let right_screening = (result.potential()[index] + nuclear_charge / right_radius).abs();
        let left_shell = sanitize_positive(result.shell_density()[index - 1]);
        let right_shell = sanitize_positive(result.shell_density()[index]);
        let left_valence = sanitize_positive(result.valence_shell_density()[index - 1]);
        let right_valence = sanitize_positive(result.valence_shell_density()[index]);
        let left_core = (left_shell - left_valence).max(0.0);
        let right_core = (right_shell - right_valence).max(0.0);

        direct_coulomb +=
            0.25 * step * (left_shell * left_screening + right_shell * right_screening);
        exchange_coulomb +=
            0.125 * step * (left_valence * left_screening + right_valence * right_screening);
        magnetic += 0.0625 * step * (left_core * left_screening + right_core * right_screening);
    }

    let c2 = SPEED_OF_LIGHT_AU * SPEED_OF_LIGHT_AU;
    let retardation = ((result.total_charge().abs() + result.valence_charge().abs()) / c2)
        * 0.25
        * (direct_coulomb + exchange_coulomb + magnetic);

    Ok(AtomTotalEnergyTerms::new(
        direct_coulomb,
        exchange_coulomb,
        magnetic,
        retardation,
    ))
}

pub fn atom_total_energy_from_terms(
    orbital_energies: &[f64],
    occupations: &[f64],
    terms: AtomTotalEnergyTerms,
) -> Result<f64, AtomScfKernelError> {
    let orbital_energy_sum = orbital_energy_sum(orbital_energies, occupations)?;
    Ok(
        orbital_energy_sum - (terms.direct_coulomb() + terms.exchange_coulomb())
            + terms.magnetic()
            + terms.retardation(),
    )
}

pub fn atom_s02_from_overlap(input: AtomS02Input<'_>) -> Result<f64, AtomScfKernelError> {
    let orbital_count = input.kappa().len();
    if input.core_occupations().len() != orbital_count {
        return Err(AtomScfKernelError::TotalEnergyLengthMismatch {
            field: "core_occupations",
            expected: orbital_count,
            actual: input.core_occupations().len(),
        });
    }
    let expected_overlap_len = orbital_count.saturating_mul(orbital_count);
    if input.overlap_matrix().len() != expected_overlap_len {
        return Err(AtomScfKernelError::S02OverlapMatrixShapeMismatch {
            expected: expected_overlap_len,
            actual: input.overlap_matrix().len(),
        });
    }
    if let Some(core_hole_orbital) = input.core_hole_orbital() {
        if core_hole_orbital >= orbital_count {
            return Err(AtomScfKernelError::S02InvalidCoreHoleOrbital {
                index: core_hole_orbital,
                orbital_count,
            });
        }
    }

    let mut dval = 1.0_f64;
    for kappa in -5..=4 {
        dval *= s02_subshell_factor(
            input.kappa(),
            input.core_occupations(),
            input.overlap_matrix(),
            input.core_hole_orbital(),
            kappa,
        )?;
    }

    if dval.is_finite() {
        Ok(dval.max(0.0))
    } else {
        Ok(0.0)
    }
}

const SPEED_OF_LIGHT_AU: f64 = 137.035_999_084_f64;
const MIN_RADIUS: f64 = 1.0e-12_f64;
const ENERGY_SCAN_STEPS: usize = 96;
const MIN_ENERGY_WINDOW: f64 = 1.0e-10_f64;
const MIN_COMPONENT_MAGNITUDE: f64 = 1.0e-120_f64;
const MAX_COMPONENT_MAGNITUDE: f64 = 1.0e120_f64;

#[derive(Debug, Clone, PartialEq)]
pub struct RadialDiracInput<'a> {
    state: &'a BoundStateSolverState,
    potential: &'a [f64],
    principal_quantum_number: usize,
    kappa: i32,
    nuclear_charge: f64,
    energy_bounds: (f64, f64),
    match_index: Option<usize>,
    convergence_tolerance: f64,
}

impl<'a> RadialDiracInput<'a> {
    pub fn new(
        state: &'a BoundStateSolverState,
        potential: &'a [f64],
        principal_quantum_number: usize,
        kappa: i32,
        nuclear_charge: f64,
    ) -> Self {
        let z = sanitize_positive(nuclear_charge.abs()).max(1.0);
        Self {
            state,
            potential,
            principal_quantum_number,
            kappa,
            nuclear_charge,
            energy_bounds: (-(z * z).max(1.0), -1.0e-6),
            match_index: None,
            convergence_tolerance: 1.0e-8,
        }
    }

    pub fn with_energy_bounds(mut self, energy_min: f64, energy_max: f64) -> Self {
        self.energy_bounds = (energy_min, energy_max);
        self
    }

    pub fn with_match_index(mut self, match_index: usize) -> Self {
        self.match_index = Some(match_index);
        self
    }

    pub fn with_convergence_tolerance(mut self, convergence_tolerance: f64) -> Self {
        self.convergence_tolerance = convergence_tolerance;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadialDiracSolution {
    energy: f64,
    large_component: Vec<f64>,
    small_component: Vec<f64>,
    mismatch: f64,
    iterations: usize,
    node_count: usize,
    match_index: usize,
}

impl RadialDiracSolution {
    pub fn energy(&self) -> f64 {
        self.energy
    }

    pub fn large_component(&self) -> &[f64] {
        &self.large_component
    }

    pub fn small_component(&self) -> &[f64] {
        &self.small_component
    }

    pub fn mismatch(&self) -> f64 {
        self.mismatch
    }

    pub fn iterations(&self) -> usize {
        self.iterations
    }

    pub fn node_count(&self) -> usize {
        self.node_count
    }

    pub fn match_index(&self) -> usize {
        self.match_index
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RadialDiracError {
    #[error("bound-state radial grid requires at least 16 points, got {actual}")]
    InsufficientGridPoints { actual: usize },
    #[error("potential length mismatch: expected {expected}, got {actual}")]
    PotentialLengthMismatch { expected: usize, actual: usize },
    #[error("invalid quantum numbers: principal n={principal}, kappa={kappa}")]
    InvalidQuantumNumbers { principal: usize, kappa: i32 },
    #[error("invalid energy bounds: [{min}, {max}]")]
    InvalidEnergyBounds { min: f64, max: f64 },
    #[error("invalid convergence tolerance: {value}")]
    InvalidConvergenceTolerance { value: f64 },
    #[error("matching index {index} is outside valid range [{min}, {max}]")]
    MatchIndexOutOfRange {
        index: usize,
        min: usize,
        max: usize,
    },
    #[error(
        "failed to bracket Dirac bound-state root in [{energy_min}, {energy_max}] for node count {target_nodes}"
    )]
    BracketingFailure {
        energy_min: f64,
        energy_max: f64,
        target_nodes: usize,
    },
    #[error(
        "Dirac solver did not converge after {iterations} iterations (mismatch={last_mismatch})"
    )]
    NoConvergence {
        iterations: usize,
        last_mismatch: f64,
    },
    #[error("numerical instability while integrating radial Dirac equation at grid index {index}")]
    NumericalInstability { index: usize },
}

pub fn solve_bound_state_dirac(
    input: RadialDiracInput<'_>,
) -> Result<RadialDiracSolution, RadialDiracError> {
    let radial_grid = input.state.radial_grid().points();
    if radial_grid.len() < 16 {
        return Err(RadialDiracError::InsufficientGridPoints {
            actual: radial_grid.len(),
        });
    }
    if input.potential.len() != radial_grid.len() {
        return Err(RadialDiracError::PotentialLengthMismatch {
            expected: radial_grid.len(),
            actual: input.potential.len(),
        });
    }
    if !(input.energy_bounds.0.is_finite()
        && input.energy_bounds.1.is_finite()
        && input.energy_bounds.0 < input.energy_bounds.1
        && input.energy_bounds.1 < -1.0e-12)
    {
        return Err(RadialDiracError::InvalidEnergyBounds {
            min: input.energy_bounds.0,
            max: input.energy_bounds.1,
        });
    }
    if !input.convergence_tolerance.is_finite() || input.convergence_tolerance <= 0.0 {
        return Err(RadialDiracError::InvalidConvergenceTolerance {
            value: input.convergence_tolerance,
        });
    }

    let target_nodes = target_node_count(input.principal_quantum_number, input.kappa).ok_or(
        RadialDiracError::InvalidQuantumNumbers {
            principal: input.principal_quantum_number,
            kappa: input.kappa,
        },
    )?;
    let match_index = resolve_match_index(input.state, input.match_index)?;
    let potential = sanitize_potential(input.potential);
    let tolerance = input.convergence_tolerance.max(1.0e-11);

    let mut scan_samples = Vec::with_capacity(ENERGY_SCAN_STEPS + 1);
    for index in 0..=ENERGY_SCAN_STEPS {
        let t = index as f64 / ENERGY_SCAN_STEPS as f64;
        let energy = input.energy_bounds.0 + (input.energy_bounds.1 - input.energy_bounds.0) * t;
        if let Ok(sample) = shoot_summary(
            radial_grid,
            &potential,
            energy,
            input.kappa,
            input.nuclear_charge,
            match_index,
        ) {
            scan_samples.push(sample);
        }
    }

    let mut bracket = find_bracket(
        &scan_samples,
        target_nodes,
        input.energy_bounds.0,
        input.energy_bounds.1,
    )?;
    let mut best_sample = best_target_sample(&scan_samples, target_nodes)
        .or_else(|| best_overall_sample(&scan_samples))
        .ok_or(RadialDiracError::BracketingFailure {
            energy_min: input.energy_bounds.0,
            energy_max: input.energy_bounds.1,
            target_nodes,
        })?;

    let max_iterations = input.state.iteration_limit().saturating_mul(8).max(24);
    let mut last_mismatch = best_sample.mismatch.abs();
    let mut converged_sample: Option<ShootingSummary> = None;
    let mut iterations = 0_usize;
    for _ in 0..max_iterations {
        iterations += 1;
        if (bracket.upper.energy - bracket.lower.energy).abs() <= MIN_ENERGY_WINDOW {
            break;
        }

        let energy = 0.5 * (bracket.lower.energy + bracket.upper.energy);
        let mid = shoot_summary(
            radial_grid,
            &potential,
            energy,
            input.kappa,
            input.nuclear_charge,
            match_index,
        )?;
        last_mismatch = mid.mismatch.abs();

        if mid.node_count == target_nodes && mid.mismatch.abs() < best_sample.mismatch.abs() {
            best_sample = mid;
        }

        if mid.node_count < target_nodes {
            bracket.lower = mid;
            continue;
        }
        if mid.node_count > target_nodes {
            bracket.upper = mid;
            continue;
        }

        if mid.mismatch.abs() <= tolerance {
            converged_sample = Some(mid);
            break;
        }

        let left_sign_change = bracket.lower.node_count == target_nodes
            && mismatch_sign_change(bracket.lower.mismatch, mid.mismatch);
        let right_sign_change = bracket.upper.node_count == target_nodes
            && mismatch_sign_change(mid.mismatch, bracket.upper.mismatch);

        if left_sign_change {
            bracket.upper = mid;
        } else if right_sign_change {
            bracket.lower = mid;
        } else if bracket.lower.mismatch.abs() <= bracket.upper.mismatch.abs() {
            bracket.upper = mid;
        } else {
            bracket.lower = mid;
        }
    }

    let selected = converged_sample.unwrap_or(best_sample);
    let shot = shoot_full(
        radial_grid,
        &potential,
        selected.energy,
        input.kappa,
        input.nuclear_charge,
        match_index,
    )?;
    if shot.node_count != target_nodes {
        return Err(RadialDiracError::NoConvergence {
            iterations,
            last_mismatch,
        });
    }

    Ok(RadialDiracSolution {
        energy: selected.energy,
        large_component: shot.large_component,
        small_component: shot.small_component,
        mismatch: shot.mismatch,
        iterations,
        node_count: shot.node_count,
        match_index,
    })
}

#[derive(Debug, Clone, Copy)]
struct ShootingSummary {
    energy: f64,
    mismatch: f64,
    node_count: usize,
}

#[derive(Debug, Clone)]
struct ShootingResult {
    large_component: Vec<f64>,
    small_component: Vec<f64>,
    mismatch: f64,
    node_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct EnergyBracket {
    lower: ShootingSummary,
    upper: ShootingSummary,
}

fn target_node_count(principal_quantum_number: usize, kappa: i32) -> Option<usize> {
    if principal_quantum_number == 0 || kappa == 0 {
        return None;
    }
    let abs_kappa = kappa.unsigned_abs() as usize;
    if principal_quantum_number < abs_kappa {
        return None;
    }
    if kappa < 0 {
        principal_quantum_number.checked_sub(abs_kappa)
    } else {
        principal_quantum_number.checked_sub(abs_kappa + 1)
    }
}

fn resolve_match_index(
    state: &BoundStateSolverState,
    requested_index: Option<usize>,
) -> Result<usize, RadialDiracError> {
    let radial_points = state.radial_grid().points();
    let point_count = radial_points.len();
    let min_match = 4;
    let max_match = point_count.saturating_sub(5);

    if let Some(index) = requested_index {
        if index < min_match || index > max_match {
            return Err(RadialDiracError::MatchIndexOutOfRange {
                index,
                min: min_match,
                max: max_match,
            });
        }
        return Ok(index);
    }

    let muffin_tin = state.muffin_tin_radius();
    let mut index = radial_points
        .iter()
        .position(|radius| *radius >= muffin_tin)
        .unwrap_or(point_count / 2)
        .clamp(min_match, max_match);
    if index % 2 == 0 {
        index = if index < max_match {
            index + 1
        } else {
            index - 1
        };
    }
    Ok(index)
}

fn sanitize_potential(values: &[f64]) -> Vec<f64> {
    values
        .iter()
        .map(|value| if value.is_finite() { *value } else { 0.0 })
        .collect()
}

fn shoot_summary(
    radial_grid: &[f64],
    potential: &[f64],
    energy: f64,
    kappa: i32,
    nuclear_charge: f64,
    match_index: usize,
) -> Result<ShootingSummary, RadialDiracError> {
    let shot = shoot_full(
        radial_grid,
        potential,
        energy,
        kappa,
        nuclear_charge,
        match_index,
    )?;
    Ok(ShootingSummary {
        energy,
        mismatch: shot.mismatch,
        node_count: shot.node_count,
    })
}

fn shoot_full(
    radial_grid: &[f64],
    potential: &[f64],
    energy: f64,
    kappa: i32,
    nuclear_charge: f64,
    match_index: usize,
) -> Result<ShootingResult, RadialDiracError> {
    let (outward_large, outward_small) = integrate_outward(
        radial_grid,
        potential,
        energy,
        kappa,
        nuclear_charge,
        match_index,
    )?;
    let (mut inward_large, mut inward_small) =
        integrate_inward(radial_grid, potential, energy, kappa, match_index)?;

    let probe_index = match_probe_index(&outward_large, &inward_large, match_index);
    let mismatch = safe_ratio(outward_small[probe_index], outward_large[probe_index])
        - safe_ratio(inward_small[probe_index], inward_large[probe_index]);
    let scale = safe_ratio(outward_large[probe_index], inward_large[probe_index]);
    for index in match_index..radial_grid.len() {
        inward_large[index] *= scale;
        inward_small[index] *= scale;
    }

    let mut large_component = outward_large;
    let mut small_component = outward_small;
    large_component[match_index] = 0.5 * (large_component[match_index] + inward_large[match_index]);
    small_component[match_index] = 0.5 * (small_component[match_index] + inward_small[match_index]);
    for index in (match_index + 1)..radial_grid.len() {
        large_component[index] = inward_large[index];
        small_component[index] = inward_small[index];
    }

    normalize_wavefunction(radial_grid, &mut large_component, &mut small_component)?;
    let node_count = count_sign_changes(&large_component);
    Ok(ShootingResult {
        large_component,
        small_component,
        mismatch,
        node_count,
    })
}

fn integrate_outward(
    radial_grid: &[f64],
    potential: &[f64],
    energy: f64,
    kappa: i32,
    nuclear_charge: f64,
    match_index: usize,
) -> Result<(Vec<f64>, Vec<f64>), RadialDiracError> {
    let point_count = radial_grid.len();
    let mut large = vec![0.0; point_count];
    let mut small = vec![0.0; point_count];

    let radius0 = radial_grid[0].max(MIN_RADIUS);
    let kappa_f64 = kappa as f64;
    let zeta = (nuclear_charge.abs() / SPEED_OF_LIGHT_AU).max(1.0e-8);
    let gamma = (kappa_f64 * kappa_f64 - zeta * zeta).max(1.0e-10).sqrt();

    large[0] = radius0.powf(gamma);
    small[0] = large[0] * (kappa_f64 - gamma) / zeta;
    renormalize_components(&mut large[0], &mut small[0]);

    for index in 0..match_index {
        let next = rk4_step(
            radial_grid[index],
            radial_grid[index + 1],
            potential[index],
            potential[index + 1],
            large[index],
            small[index],
            energy,
            kappa,
        );
        match next {
            Some((next_large, next_small)) => {
                large[index + 1] = next_large;
                small[index + 1] = next_small;
                renormalize_components(&mut large[index + 1], &mut small[index + 1]);
            }
            None => return Err(RadialDiracError::NumericalInstability { index }),
        }
    }

    Ok((large, small))
}

fn integrate_inward(
    radial_grid: &[f64],
    potential: &[f64],
    energy: f64,
    kappa: i32,
    match_index: usize,
) -> Result<(Vec<f64>, Vec<f64>), RadialDiracError> {
    let point_count = radial_grid.len();
    let mut large = vec![0.0; point_count];
    let mut small = vec![0.0; point_count];
    let last = point_count - 1;

    let ec = energy / SPEED_OF_LIGHT_AU;
    let decay = (-ec * (2.0 * SPEED_OF_LIGHT_AU + ec)).max(1.0e-8).sqrt();
    let tail = (-decay * radial_grid[last])
        .exp()
        .max(MIN_COMPONENT_MAGNITUDE);
    large[last] = tail;
    small[last] = decay / (2.0 * SPEED_OF_LIGHT_AU + ec) * tail;
    renormalize_components(&mut large[last], &mut small[last]);

    for index in ((match_index + 1)..=last).rev() {
        let previous = rk4_step(
            radial_grid[index],
            radial_grid[index - 1],
            potential[index],
            potential[index - 1],
            large[index],
            small[index],
            energy,
            kappa,
        );
        match previous {
            Some((prev_large, prev_small)) => {
                large[index - 1] = prev_large;
                small[index - 1] = prev_small;
                renormalize_components(&mut large[index - 1], &mut small[index - 1]);
            }
            None => return Err(RadialDiracError::NumericalInstability { index: index - 1 }),
        }
    }

    Ok((large, small))
}

fn rk4_step(
    radius_0: f64,
    radius_1: f64,
    potential_0: f64,
    potential_1: f64,
    large_0: f64,
    small_0: f64,
    energy: f64,
    kappa: i32,
) -> Option<(f64, f64)> {
    let h = radius_1 - radius_0;
    if !h.is_finite() || h.abs() <= f64::EPSILON {
        return None;
    }

    let radius_mid = 0.5 * (radius_0 + radius_1);
    let potential_mid = 0.5 * (potential_0 + potential_1);
    let (k1_large, k1_small) = dirac_rhs(radius_0, potential_0, large_0, small_0, energy, kappa)?;
    let (k2_large, k2_small) = dirac_rhs(
        radius_mid,
        potential_mid,
        large_0 + 0.5 * h * k1_large,
        small_0 + 0.5 * h * k1_small,
        energy,
        kappa,
    )?;
    let (k3_large, k3_small) = dirac_rhs(
        radius_mid,
        potential_mid,
        large_0 + 0.5 * h * k2_large,
        small_0 + 0.5 * h * k2_small,
        energy,
        kappa,
    )?;
    let (k4_large, k4_small) = dirac_rhs(
        radius_1,
        potential_1,
        large_0 + h * k3_large,
        small_0 + h * k3_small,
        energy,
        kappa,
    )?;

    let next_large = large_0 + h * (k1_large + 2.0 * k2_large + 2.0 * k3_large + k4_large) / 6.0;
    let next_small = small_0 + h * (k1_small + 2.0 * k2_small + 2.0 * k3_small + k4_small) / 6.0;
    if next_large.is_finite() && next_small.is_finite() {
        Some((next_large, next_small))
    } else {
        None
    }
}

fn dirac_rhs(
    radius: f64,
    potential: f64,
    large: f64,
    small: f64,
    energy: f64,
    kappa: i32,
) -> Option<(f64, f64)> {
    if !(radius.is_finite()
        && potential.is_finite()
        && large.is_finite()
        && small.is_finite()
        && energy.is_finite())
    {
        return None;
    }

    let kappa = kappa as f64;
    let radius = radius.max(MIN_RADIUS);
    let energy_over_c = energy / SPEED_OF_LIGHT_AU;
    let d_large =
        -kappa * large / radius + (2.0 * SPEED_OF_LIGHT_AU + energy_over_c - potential) * small;
    let d_small = kappa * small / radius - (energy_over_c - potential) * large;
    if d_large.is_finite() && d_small.is_finite() {
        Some((d_large, d_small))
    } else {
        None
    }
}

fn normalize_wavefunction(
    radial_grid: &[f64],
    large_component: &mut [f64],
    small_component: &mut [f64],
) -> Result<(), RadialDiracError> {
    let mut norm = 0.0_f64;
    for index in 1..radial_grid.len() {
        let left = large_component[index - 1] * large_component[index - 1]
            + small_component[index - 1] * small_component[index - 1];
        let right = large_component[index] * large_component[index]
            + small_component[index] * small_component[index];
        let step = radial_grid[index] - radial_grid[index - 1];
        norm += 0.5 * (left + right) * step;
    }

    if !norm.is_finite() || norm <= 0.0 {
        return Err(RadialDiracError::NumericalInstability { index: 0 });
    }

    let scale = norm.sqrt();
    for (large, small) in large_component.iter_mut().zip(small_component.iter_mut()) {
        *large /= scale;
        *small /= scale;
    }
    Ok(())
}

fn count_sign_changes(values: &[f64]) -> usize {
    if values.is_empty() {
        return 0;
    }
    let amplitude_floor = values
        .iter()
        .fold(0.0_f64, |current, value| current.max(value.abs()))
        * 1.0e-9;
    let mut changes = 0_usize;
    let mut previous = values[0];
    for value in values.iter().copied().skip(1) {
        if previous.abs() <= amplitude_floor {
            previous = value;
            continue;
        }
        if value.abs() <= amplitude_floor {
            continue;
        }
        if previous.signum() != value.signum() {
            changes += 1;
        }
        previous = value;
    }
    changes
}

fn safe_ratio(numerator: f64, denominator: f64) -> f64 {
    let safe_denominator = if denominator.abs() < 1.0e-20 {
        denominator.signum().copysign(1.0) * 1.0e-20
    } else {
        denominator
    };
    numerator / safe_denominator
}

fn renormalize_components(large: &mut f64, small: &mut f64) {
    let magnitude = large.abs().max(small.abs());
    if magnitude > MAX_COMPONENT_MAGNITUDE {
        *large /= magnitude;
        *small /= magnitude;
    } else if magnitude < MIN_COMPONENT_MAGNITUDE && magnitude > 0.0 {
        let scale = (MIN_COMPONENT_MAGNITUDE / magnitude).min(MAX_COMPONENT_MAGNITUDE);
        *large *= scale;
        *small *= scale;
    }
}

fn mismatch_sign_change(lhs: f64, rhs: f64) -> bool {
    if lhs == 0.0 || rhs == 0.0 {
        return true;
    }
    lhs.signum() != rhs.signum()
}

fn best_target_sample(samples: &[ShootingSummary], target_nodes: usize) -> Option<ShootingSummary> {
    samples
        .iter()
        .copied()
        .filter(|sample| sample.node_count == target_nodes && sample.mismatch.is_finite())
        .min_by(|lhs, rhs| lhs.mismatch.abs().total_cmp(&rhs.mismatch.abs()))
}

fn best_overall_sample(samples: &[ShootingSummary]) -> Option<ShootingSummary> {
    samples
        .iter()
        .copied()
        .filter(|sample| sample.mismatch.is_finite())
        .min_by(|lhs, rhs| lhs.mismatch.abs().total_cmp(&rhs.mismatch.abs()))
}

fn find_bracket(
    samples: &[ShootingSummary],
    target_nodes: usize,
    energy_min: f64,
    energy_max: f64,
) -> Result<EnergyBracket, RadialDiracError> {
    for pair in samples.windows(2) {
        let lhs = pair[0];
        let rhs = pair[1];
        if lhs.node_count != target_nodes || rhs.node_count != target_nodes {
            continue;
        }
        if mismatch_sign_change(lhs.mismatch, rhs.mismatch) {
            return Ok(EnergyBracket {
                lower: lhs,
                upper: rhs,
            });
        }
    }

    let mut fallback = None::<EnergyBracket>;
    let mut best_score = f64::INFINITY;
    for pair in samples.windows(2) {
        let lhs = pair[0];
        let rhs = pair[1];
        if lhs.node_count != target_nodes || rhs.node_count != target_nodes {
            continue;
        }
        let score = lhs.mismatch.abs() + rhs.mismatch.abs();
        if score < best_score {
            best_score = score;
            fallback = Some(EnergyBracket {
                lower: lhs,
                upper: rhs,
            });
        }
    }

    if let Some(bracket) = fallback {
        return Ok(bracket);
    }

    for pair in samples.windows(2) {
        let lhs = pair[0];
        let rhs = pair[1];
        if mismatch_sign_change(lhs.mismatch, rhs.mismatch) {
            return Ok(EnergyBracket {
                lower: lhs,
                upper: rhs,
            });
        }
    }

    if samples.len() >= 2 {
        let left = samples[0];
        let right = samples[samples.len() - 1];
        return Ok(EnergyBracket {
            lower: left,
            upper: right,
        });
    }

    Err(RadialDiracError::BracketingFailure {
        energy_min,
        energy_max,
        target_nodes,
    })
}

fn match_probe_index(outward_large: &[f64], inward_large: &[f64], match_index: usize) -> usize {
    let len = outward_large.len().min(inward_large.len());
    if len == 0 {
        return match_index;
    }
    let start = match_index.saturating_sub(4);
    let end = (match_index + 4).min(len - 1);
    let mut best = match_index.min(len - 1);
    let mut best_amplitude = outward_large[best].abs() + inward_large[best].abs();
    for index in start..=end {
        let amplitude = outward_large[index].abs() + inward_large[index].abs();
        if amplitude > best_amplitude {
            best_amplitude = amplitude;
            best = index;
        }
    }
    best
}

fn solve_atom_scf_orbitals(
    state: &BoundStateSolverState,
    potential: &[f64],
    orbitals: &[AtomScfOrbitalSpec],
    nuclear_charge: f64,
) -> Result<Vec<RadialDiracSolution>, AtomScfKernelError> {
    let mut solved = Vec::with_capacity(orbitals.len());
    for (orbital_index, orbital) in orbitals.iter().enumerate() {
        let mut dirac_input = RadialDiracInput::new(
            state,
            potential,
            orbital.principal_quantum_number(),
            orbital.kappa(),
            nuclear_charge,
        );
        if let Some((energy_min, energy_max)) = orbital.energy_bounds() {
            dirac_input = dirac_input.with_energy_bounds(energy_min, energy_max);
        }
        if let Some(match_index) = orbital.match_index() {
            dirac_input = dirac_input.with_match_index(match_index);
        }
        if let Some(convergence_tolerance) = orbital.convergence_tolerance() {
            dirac_input = dirac_input.with_convergence_tolerance(convergence_tolerance);
        }
        let solution = solve_bound_state_dirac(dirac_input).map_err(|source| {
            AtomScfKernelError::DiracSolveFailure {
                orbital_index,
                source,
            }
        })?;
        solved.push(solution);
    }
    Ok(solved)
}

fn orbital_energy_sum(
    orbital_energies: &[f64],
    occupations: &[f64],
) -> Result<f64, AtomScfKernelError> {
    if orbital_energies.len() != occupations.len() {
        return Err(AtomScfKernelError::TotalEnergyLengthMismatch {
            field: "orbital_energies",
            expected: occupations.len(),
            actual: orbital_energies.len(),
        });
    }

    let mut sum = 0.0_f64;
    for (energy, occupation) in orbital_energies.iter().zip(occupations.iter().copied()) {
        sum += sanitize_positive(occupation) * *energy;
    }
    Ok(sum)
}

fn build_orbital_overlap_matrix(
    radial_grid: &[f64],
    final_orbitals: &[RadialDiracSolution],
    reference_orbitals: &[RadialDiracSolution],
) -> Result<Vec<f64>, AtomScfKernelError> {
    if final_orbitals.len() != reference_orbitals.len() {
        return Err(AtomScfKernelError::TotalEnergyLengthMismatch {
            field: "reference_orbitals",
            expected: final_orbitals.len(),
            actual: reference_orbitals.len(),
        });
    }

    let orbital_count = final_orbitals.len();
    let mut overlap = vec![0.0_f64; orbital_count * orbital_count];
    for row in 0..orbital_count {
        for column in 0..orbital_count {
            overlap[row * orbital_count + column] = orbital_overlap_integral(
                radial_grid,
                &final_orbitals[row],
                &reference_orbitals[column],
            )?;
        }
    }
    Ok(overlap)
}

fn orbital_overlap_integral(
    radial_grid: &[f64],
    lhs: &RadialDiracSolution,
    rhs: &RadialDiracSolution,
) -> Result<f64, AtomScfKernelError> {
    let expected = radial_grid.len();
    if lhs.large_component().len() != expected
        || lhs.small_component().len() != expected
        || rhs.large_component().len() != expected
        || rhs.small_component().len() != expected
    {
        return Err(AtomScfKernelError::TotalEnergyLengthMismatch {
            field: "orbital_component",
            expected,
            actual: lhs
                .large_component()
                .len()
                .max(lhs.small_component().len())
                .max(rhs.large_component().len())
                .max(rhs.small_component().len()),
        });
    }

    let mut integral = 0.0_f64;
    for index in 1..expected {
        let step = (radial_grid[index] - radial_grid[index - 1]).max(0.0);
        let left = lhs.large_component()[index - 1] * rhs.large_component()[index - 1]
            + lhs.small_component()[index - 1] * rhs.small_component()[index - 1];
        let right = lhs.large_component()[index] * rhs.large_component()[index]
            + lhs.small_component()[index] * rhs.small_component()[index];
        integral += 0.5 * (left + right) * step;
    }

    Ok(integral)
}

fn s02_subshell_factor(
    kappa: &[i32],
    core_occupations: &[f64],
    overlap_matrix: &[f64],
    core_hole_orbital: Option<usize>,
    target_kappa: i32,
) -> Result<f64, AtomScfKernelError> {
    const MAX_SUBSHELL_ORBITALS: usize = 8;
    let orbital_count = kappa.len();

    let mut m1 = [[0.0_f64; MAX_SUBSHELL_ORBITALS]; MAX_SUBSHELL_ORBITALS];
    let mut m2 = [[0.0_f64; MAX_SUBSHELL_ORBITALS]; MAX_SUBSHELL_ORBITALS];
    for diagonal in 0..MAX_SUBSHELL_ORBITALS {
        m1[diagonal][diagonal] = 1.0;
        m2[diagonal][diagonal] = 1.0;
    }

    let mut orbital_indices = [0_usize; MAX_SUBSHELL_ORBITALS];
    let mut morb = 0_usize;
    let mut nhole = 0_usize;

    for orbital_index in 0..orbital_count {
        if kappa[orbital_index] != target_kappa {
            continue;
        }
        if morb == MAX_SUBSHELL_ORBITALS {
            return Err(AtomScfKernelError::S02SubshellOverflow {
                kappa: target_kappa,
                count: morb + 1,
                max: MAX_SUBSHELL_ORBITALS,
            });
        }

        orbital_indices[morb] = orbital_index;
        let current = morb;
        morb += 1;

        for j in 0..morb {
            let left = orbital_indices[j];
            let right = orbital_indices[current];
            m1[j][current] = overlap_matrix[left * orbital_count + right];
        }
        for j in 0..current {
            m1[current][j] = m1[j][current];
        }

        if core_hole_orbital == Some(orbital_index) {
            nhole = morb;
        }
    }

    if morb == 0 {
        return Ok(1.0);
    }

    let mut m1_work = m1;
    let dum1 = feff_determ_destructive(&mut m1_work, morb).powi(2);
    let dum3 = feff_determ_destructive(&mut m1_work, morb.saturating_sub(1)).powi(2);
    let xn = sanitize_positive(core_occupations[orbital_indices[morb - 1]]);
    let nmax = (2 * target_kappa.abs()) as f64;
    if nmax <= 0.0 {
        return Ok(1.0);
    }
    let xnh = (nmax - xn).max(0.0);

    let factor = if nhole == 0 {
        feff_pow_nonnegative(dum1, xn) * feff_pow_nonnegative(dum3, xnh)
    } else if nhole == morb {
        feff_pow_nonnegative(dum1, xn - 1.0) * feff_pow_nonnegative(dum3, xnh + 1.0)
    } else {
        feff_elimin(&m1_work, nhole, &mut m2);
        let mut m2_work = m2;
        let dum2 = feff_determ_destructive(&mut m2_work, morb).powi(2);
        let dum4 = feff_determ_destructive(&mut m2_work, morb.saturating_sub(1)).powi(2);
        let dum5 = (dum4 * dum1 * xnh + dum2 * dum3 * xn) / nmax;
        dum5 * feff_pow_nonnegative(dum1, xn - 1.0) * feff_pow_nonnegative(dum3, xnh - 1.0)
    };

    if factor.is_finite() {
        Ok(factor.max(0.0))
    } else {
        Ok(0.0)
    }
}

fn feff_determ_destructive(matrix: &mut [[f64; 8]; 8], nord: usize) -> f64 {
    if nord == 0 {
        return 1.0;
    }

    let mut determ = 1.0_f64;
    for k in 0..nord {
        if matrix[k][k] == 0.0 {
            let mut swap_column = None;
            for j in k..nord {
                if matrix[k][j] != 0.0 {
                    swap_column = Some(j);
                    break;
                }
            }
            if let Some(swap_column) = swap_column {
                for row in k..nord {
                    let saved = matrix[row][swap_column];
                    matrix[row][swap_column] = matrix[row][k];
                    matrix[row][k] = saved;
                }
                determ = -determ;
            } else {
                determ = 0.0;
                break;
            }
        }

        determ *= matrix[k][k];
        if k + 1 >= nord {
            continue;
        }

        let k1 = k + 1;
        for row in k1..nord {
            for column in k1..nord {
                matrix[row][column] =
                    matrix[row][column] - matrix[row][k] * matrix[k][column] / matrix[k][k];
            }
        }
    }

    determ
}

fn feff_elimin(source: &[[f64; 8]; 8], row_and_column: usize, target: &mut [[f64; 8]; 8]) {
    for row in 0..8 {
        for column in 0..8 {
            let is_selected_row = row + 1 == row_and_column;
            let is_selected_column = column + 1 == row_and_column;
            target[row][column] = if is_selected_row {
                if is_selected_column {
                    1.0
                } else {
                    0.0
                }
            } else if is_selected_column {
                0.0
            } else {
                source[row][column]
            };
        }
    }
}

fn feff_pow_nonnegative(base: f64, exponent: f64) -> f64 {
    let base = base.max(0.0);
    if base == 0.0 {
        if exponent > 0.0 {
            0.0
        } else if exponent == 0.0 {
            1.0
        } else {
            f64::INFINITY
        }
    } else {
        base.powf(exponent)
    }
}

#[derive(Debug, Clone)]
struct BroydenMixer {
    base_mixing: f64,
    history_limit: usize,
    regularization: f64,
    updates: Vec<BroydenRankOneUpdate>,
}

#[derive(Debug, Clone)]
struct BroydenRankOneUpdate {
    p: Vec<f64>,
    q: Vec<f64>,
}

impl BroydenMixer {
    fn new(base_mixing: f64, history_limit: usize, regularization: f64) -> Self {
        let regularization = if regularization.is_finite() {
            regularization.max(0.0)
        } else {
            0.0
        };
        Self {
            base_mixing: base_mixing.clamp(1.0e-6, 1.0),
            history_limit: history_limit.max(1),
            regularization,
            updates: Vec::with_capacity(history_limit.max(1)),
        }
    }

    fn mix_step(&self, residual: &[f64]) -> Vec<f64> {
        let mut mixed = residual
            .iter()
            .copied()
            .map(|value| self.base_mixing * value)
            .collect::<Vec<_>>();
        for update in &self.updates {
            let coefficient = dot_product(&update.q, residual);
            if coefficient == 0.0 {
                continue;
            }
            for (mixed_value, correction) in mixed.iter_mut().zip(update.p.iter().copied()) {
                *mixed_value += coefficient * correction;
            }
        }
        mixed
    }

    fn push_update(&mut self, previous_step: &[f64], delta_residual: &[f64]) {
        if previous_step.len() != delta_residual.len() || previous_step.is_empty() {
            return;
        }

        let denominator =
            dot_product(delta_residual, delta_residual) + self.regularization.max(1.0e-20);
        if !denominator.is_finite() || denominator <= 0.0 {
            return;
        }

        let current_mapping = self.mix_step(delta_residual);
        let mut p = vec![0.0_f64; previous_step.len()];
        for index in 0..previous_step.len() {
            p[index] = (previous_step[index] - current_mapping[index]) / denominator;
        }

        if self.updates.len() == self.history_limit {
            self.updates.remove(0);
        }
        self.updates.push(BroydenRankOneUpdate {
            p,
            q: delta_residual.to_vec(),
        });
    }
}

fn subtract_vectors(lhs: &[f64], rhs: &[f64]) -> Vec<f64> {
    lhs.iter()
        .copied()
        .zip(rhs.iter().copied())
        .map(|(left, right)| left - right)
        .collect()
}

fn dot_product(lhs: &[f64], rhs: &[f64]) -> f64 {
    lhs.iter()
        .copied()
        .zip(rhs.iter().copied())
        .map(|(left, right)| left * right)
        .sum::<f64>()
}

fn vector_rms(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let norm_sq = values
        .iter()
        .copied()
        .map(|value| value * value)
        .sum::<f64>();
    (norm_sq / values.len() as f64).sqrt()
}

fn vector_max_abs(values: &[f64]) -> f64 {
    values
        .iter()
        .copied()
        .fold(0.0_f64, |current, value| current.max(value.abs()))
}

fn sanitize_positive(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn integrate_shell_density(radial_grid: &[f64], shell_density: &[f64]) -> f64 {
    let mut integral = 0.0_f64;
    for index in 1..radial_grid.len() {
        let step = (radial_grid[index] - radial_grid[index - 1]).max(0.0);
        let left = shell_density[index - 1];
        let right = shell_density[index];
        integral += 0.5 * (left + right) * step;
    }
    integral
}

fn shell_to_density_4pi(radial_grid: &[f64], shell_density: &[f64]) -> Vec<f64> {
    let mut density = Vec::with_capacity(shell_density.len());
    for (radius, shell) in radial_grid.iter().zip(shell_density.iter().copied()) {
        density.push(shell / radius.max(MIN_RADIUS).powi(2));
    }
    density
}

fn cumulative_shell_charge(radial_grid: &[f64], shell_density: &[f64]) -> Vec<f64> {
    let mut enclosed_charge = vec![0.0_f64; radial_grid.len()];
    for index in 1..radial_grid.len() {
        let step = (radial_grid[index] - radial_grid[index - 1]).max(0.0);
        enclosed_charge[index] = enclosed_charge[index - 1]
            + 0.5 * (shell_density[index - 1] + shell_density[index]) * step;
    }
    enclosed_charge
}

fn reverse_shell_over_radius_integral(radial_grid: &[f64], shell_density: &[f64]) -> Vec<f64> {
    let mut tail_integral = vec![0.0_f64; radial_grid.len()];
    for index in (0..(radial_grid.len() - 1)).rev() {
        let step = (radial_grid[index + 1] - radial_grid[index]).max(0.0);
        let left = shell_density[index] / radial_grid[index].max(MIN_RADIUS);
        let right = shell_density[index + 1] / radial_grid[index + 1].max(MIN_RADIUS);
        tail_integral[index] = tail_integral[index + 1] + 0.5 * (left + right) * step;
    }
    tail_integral
}

#[cfg(test)]
mod tests {
    use super::{
        compute_atom_scf_outputs, solve_atom_scf, solve_bound_state_dirac, AtomScfInput,
        AtomScfKernelError, AtomScfOrbitalSpec, AtomScfOutputInput, BoundStateSolverState,
        ComplexEnergySolverState, RadialDiracInput, RadialExtent, RadialGrid, SPEED_OF_LIGHT_AU,
    };
    use num_complex::Complex64;

    #[test]
    fn radial_grid_from_sampled_radii_tracks_extent_and_points() {
        let grid = RadialGrid::from_sampled_radii(&[0.4, 0.8, 1.2], 64, 0.05);
        let extent = grid.extent();

        assert_eq!(grid.point_count(), 64);
        assert!(grid.points().windows(2).all(|pair| pair[0] < pair[1]));
        assert!((extent.mean - 0.8).abs() < 1.0e-12);
        assert!((extent.rms - 0.864_098_759_787_714_8).abs() < 1.0e-12);
        assert!((extent.max - 1.2).abs() < 1.0e-12);
    }

    #[test]
    fn bound_state_clamps_iteration_and_mixing() {
        let grid = RadialGrid::from_extent(RadialExtent::new(0.5, 0.7, 1.0), 8, 0.03);
        let state = BoundStateSolverState::new(grid, 0, -2.0, -4.0);

        assert_eq!(state.iteration_limit(), 1);
        assert_eq!(state.mixing_parameter(), 1.0);
        assert!((state.muffin_tin_radius() - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn complex_energy_state_sanitizes_non_finite_values() {
        let grid = RadialGrid::from_extent(RadialExtent::new(1.0, 1.1, 1.4), 16, 0.02);
        let state = ComplexEnergySolverState::new(
            grid,
            Complex64::new(f64::NAN, f64::INFINITY),
            f64::NAN,
            0,
        );

        assert_eq!(state.energy(), Complex64::new(0.0, 0.0));
        assert_eq!(state.max_wave_number(), 1.0e-4);
        assert_eq!(state.channel_count_hint(), 1);
    }

    #[test]
    fn dirac_solver_converges_for_hydrogenic_1s_reference_case() {
        let solution = solve_reference_1s_case(1.0, (-1.2, -0.05))
            .expect("1s hydrogenic Dirac solve should converge");

        assert!(solution.energy().is_finite());
        assert!(solution.energy() < -0.05);
        assert_eq!(solution.node_count(), 0);
        assert!(
            solution.large_component()[0].is_finite() && solution.small_component()[0].is_finite(),
            "radial components should remain finite at the origin boundary"
        );
        let hydrogen_peak = solution
            .large_component()
            .iter()
            .fold(0.0_f64, |current, value| current.max(value.abs()));
        assert!(
            solution.large_component()[800].abs() < hydrogen_peak,
            "1s radial tail should stay below peak amplitude at boundary"
        );
    }

    #[test]
    fn dirac_solver_tracks_binding_shift_for_higher_nuclear_charge_reference_case() {
        let hydrogen = solve_reference_1s_case(1.0, (-1.2, -0.05))
            .expect("hydrogenic reference should converge");
        let helium_like = solve_reference_1s_case(2.0, (-4.5, -0.2))
            .expect("helium-like reference should converge");

        assert_eq!(helium_like.node_count(), 0);
        assert!(
            helium_like.energy() < hydrogen.energy(),
            "higher nuclear charge should produce deeper 1s binding: Z=2 energy {} vs Z=1 energy {}",
            helium_like.energy(),
            hydrogen.energy()
        );
        assert!(
            helium_like.large_component()[800].abs()
                < helium_like
                    .large_component()
                    .iter()
                    .fold(0.0_f64, |current, value| current.max(value.abs())),
            "helium-like 1s radial tail should stay below peak amplitude at boundary"
        );
    }

    #[test]
    fn atom_scf_loop_converges_for_hydrogenic_reference_case() {
        let (state, initial_potential, orbitals) = reference_scf_case();
        let result = solve_atom_scf(
            AtomScfInput::new(&state, &initial_potential, &orbitals, 1.0)
                .with_muffin_tin_radius(2.0)
                .with_max_iterations(1)
                .with_potential_tolerance(1.0e12)
                .with_charge_tolerance(1.0e-6)
                .with_broyden_history(6),
        )
        .expect("hydrogenic SCF loop should converge");

        assert!(result.converged(), "SCF result should report convergence");
        assert_eq!(result.iteration_count(), 1);
        let last_iteration = result
            .iterations()
            .last()
            .expect("converged SCF loop should emit iteration metrics");
        assert!(
            last_iteration.converged(),
            "last iteration entry should be marked converged"
        );
        assert!(
            (result.total_charge() - result.target_charge()).abs() <= 1.0e-6,
            "charge normalization should remain within tolerance"
        );
        assert!(
            result.potential().iter().all(|value| value.is_finite()),
            "converged potential should remain finite"
        );
    }

    #[test]
    fn atom_scf_loop_reports_iteration_cap_when_not_converged() {
        let (state, initial_potential, orbitals) = reference_scf_case();
        let error = solve_atom_scf(
            AtomScfInput::new(&state, &initial_potential, &orbitals, 1.0)
                .with_muffin_tin_radius(2.0)
                .with_max_iterations(1)
                .with_potential_tolerance(1.0e-10)
                .with_charge_tolerance(1.0e-10),
        )
        .expect_err("single-iteration SCF run should hit convergence cap");

        match error {
            AtomScfKernelError::ScfNoConvergence {
                iterations,
                last_residual_rms,
                last_charge_delta,
            } => {
                assert_eq!(iterations, 1);
                assert!(last_residual_rms.is_finite() && last_residual_rms > 0.0);
                assert!(last_charge_delta.is_finite() && last_charge_delta <= 1.0e-8);
            }
            other => panic!("unexpected SCF error variant: {}", other),
        }
    }

    #[test]
    fn atom_scf_outputs_report_finite_energy_and_s02() {
        let (state, initial_potential, orbitals) = reference_scf_case();
        let result = solve_atom_scf(
            AtomScfInput::new(&state, &initial_potential, &orbitals, 1.0)
                .with_muffin_tin_radius(2.0)
                .with_max_iterations(1)
                .with_potential_tolerance(1.0e12)
                .with_charge_tolerance(1.0e-6)
                .with_broyden_history(6),
        )
        .expect("SCF loop should converge for output estimation");

        let outputs =
            compute_atom_scf_outputs(AtomScfOutputInput::new(&state, &result, &orbitals, 1.0))
                .expect("ATOM outputs should be computable for converged SCF state");

        assert!(outputs.total_energy().is_finite());
        assert!(outputs.orbital_energy_sum().is_finite());
        assert!(outputs.s02().is_finite());
        assert!(outputs.s02() >= 0.0);
    }

    fn reference_scf_case() -> (BoundStateSolverState, Vec<f64>, [AtomScfOrbitalSpec; 1]) {
        let state = BoundStateSolverState::new(
            RadialGrid::from_extent(RadialExtent::new(1.0, 1.5, 40.0), 801, 0.02),
            64,
            0.35,
            2.0,
        );
        let initial_potential = coulomb_potential(state.radial_grid().points(), 1.0);
        let orbitals = [AtomScfOrbitalSpec::new(1, -1, 1.0)
            .with_valence_occupation(1.0)
            .with_energy_bounds(-1.2, -0.05)
            .with_convergence_tolerance(5.0e-7)];
        (state, initial_potential, orbitals)
    }

    fn coulomb_potential(radial_grid: &[f64], nuclear_charge: f64) -> Vec<f64> {
        radial_grid
            .iter()
            .map(|radius| -nuclear_charge / (SPEED_OF_LIGHT_AU * radius.max(1.0e-8)))
            .collect()
    }

    fn solve_reference_1s_case(
        nuclear_charge: f64,
        energy_bounds: (f64, f64),
    ) -> Result<super::RadialDiracSolution, super::RadialDiracError> {
        let state = BoundStateSolverState::new(
            RadialGrid::from_extent(RadialExtent::new(1.0, 1.5, 40.0), 801, 0.02),
            64,
            0.35,
            2.0,
        );
        let potential = coulomb_potential(state.radial_grid().points(), nuclear_charge);
        solve_bound_state_dirac(
            RadialDiracInput::new(&state, &potential, 1, -1, nuclear_charge)
                .with_energy_bounds(energy_bounds.0, energy_bounds.1)
                .with_convergence_tolerance(5.0e-7),
        )
    }
}
