use super::dsordc::{DsordcError, DsordcInput, dsordc};
use num_complex::Complex64;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum OrtdacError {
    #[error("ps/qs/dr inputs must share the same non-zero length")]
    WaveLengthMismatch,
    #[error("aps/aqs inputs must share the same non-zero length")]
    ExpansionLengthMismatch,
    #[error("orbital channel shape mismatch")]
    OrbitalShapeMismatch,
    #[error(transparent)]
    Dsordc(#[from] DsordcError),
}

#[derive(Debug, Clone)]
pub struct OrtdacOrbital<'a> {
    pub kap: i32,
    pub occupancy: f64,
    pub fl: f64,
    pub cg: &'a [Complex64],
    pub cp: &'a [Complex64],
    pub bg: &'a [Complex64],
    pub bp: &'a [Complex64],
}

pub struct OrtdacState<'a> {
    pub ikap: i32,
    pub ps: &'a mut [Complex64],
    pub qs: &'a mut [Complex64],
    pub aps: &'a mut [Complex64],
    pub aqs: &'a mut [Complex64],
    pub dr: &'a [f64],
    pub hx: f64,
}

pub fn ortdac(
    state: &mut OrtdacState<'_>,
    orbitals: &[OrtdacOrbital<'_>],
) -> Result<(), OrtdacError> {
    if state.ps.is_empty() || state.ps.len() != state.qs.len() || state.ps.len() != state.dr.len() {
        return Err(OrtdacError::WaveLengthMismatch);
    }
    if state.aps.is_empty() || state.aps.len() != state.aqs.len() {
        return Err(OrtdacError::ExpansionLengthMismatch);
    }

    for orbital in orbitals {
        if orbital.kap != state.ikap || orbital.occupancy <= 0.0 {
            continue;
        }
        if orbital.cg.len() != state.ps.len() || orbital.cp.len() != state.ps.len() {
            return Err(OrtdacError::OrbitalShapeMismatch);
        }
        if orbital.bg.is_empty() || orbital.bp.is_empty() {
            return Err(OrtdacError::OrbitalShapeMismatch);
        }

        let ndor = state.aps.len().min(orbital.bg.len()).min(orbital.bp.len());
        let coefficient = dsordc(&DsordcInput {
            a: 0.0,
            fl_j: orbital.fl,
            dg: state.ps,
            dp: state.qs,
            cg_j: orbital.cg,
            cp_j: orbital.cp,
            dr: state.dr,
            hx: state.hx,
            ag: state.aps,
            bg_j: orbital.bg,
            ap: state.aqs,
            bp_j: orbital.bp,
            ndor,
        })?;

        for i in 0..state.ps.len() {
            state.ps[i] -= coefficient * orbital.cg[i];
            state.qs[i] -= coefficient * orbital.cp[i];
        }
        for i in 0..ndor {
            state.aps[i] -= coefficient * orbital.bg[i];
            state.aqs[i] -= coefficient * orbital.bp[i];
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{OrtdacError, OrtdacOrbital, OrtdacState, ortdac};
    use num_complex::Complex64;

    fn c(value: f64) -> Complex64 {
        Complex64::new(value, 0.0)
    }

    #[test]
    fn ortdac_orthogonalizes_matching_orbitals() {
        let mut ps = vec![c(1.0), c(1.0), c(1.0), c(1.0), c(1.0)];
        let mut qs = vec![c(0.5), c(0.5), c(0.5), c(0.5), c(0.5)];
        let mut aps = vec![c(0.2), c(0.1)];
        let mut aqs = vec![c(0.1), c(0.2)];
        let dr = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let orbital = OrtdacOrbital {
            kap: 1,
            occupancy: 2.0,
            fl: 0.0,
            cg: &[c(0.8), c(0.7), c(0.6), c(0.5), c(0.4)],
            cp: &[c(0.1), c(0.1), c(0.1), c(0.1), c(0.1)],
            bg: &[c(0.2), c(0.3)],
            bp: &[c(0.1), c(0.1)],
        };

        let original_ps = ps.clone();
        let original_qs = qs.clone();
        let original_aps = aps.clone();
        let original_aqs = aqs.clone();
        let mut state = OrtdacState {
            ikap: 1,
            ps: &mut ps,
            qs: &mut qs,
            aps: &mut aps,
            aqs: &mut aqs,
            dr: &dr,
            hx: 1.0,
        };
        ortdac(&mut state, &[orbital]).expect("valid ortdac input should succeed");
        assert_ne!(ps, original_ps);
        assert_ne!(qs, original_qs);
        assert_ne!(aps, original_aps);
        assert_ne!(aqs, original_aqs);
    }

    #[test]
    fn ortdac_requires_matching_shapes() {
        let mut ps = vec![c(1.0), c(1.0), c(1.0)];
        let mut qs = vec![c(1.0), c(1.0)];
        let mut aps = vec![c(0.0)];
        let mut aqs = vec![c(0.0)];
        let dr = vec![1.0, 2.0, 3.0];

        let mut state = OrtdacState {
            ikap: 1,
            ps: &mut ps,
            qs: &mut qs,
            aps: &mut aps,
            aqs: &mut aqs,
            dr: &dr,
            hx: 1.0,
        };

        let error = ortdac(&mut state, &[]).expect_err("shape mismatch should fail");
        assert_eq!(error, OrtdacError::WaveLengthMismatch);
    }
}
