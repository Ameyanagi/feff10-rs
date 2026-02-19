pub const ORBITAL_CAPACITY: usize = 41;
pub const IORB_CAPACITY: usize = 10;
pub const EPS_CAPACITY: usize = 820;

const DEFAULT_NUC: i32 = 11;
const DEFAULT_NES: i32 = 50;
const DEFAULT_NDOR: usize = 10;
const DEFAULT_IDIM: usize = 251;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InmuatInput {
    pub nz: f64,
    pub ihole: i32,
    pub xionin: f64,
    pub iunf: i32,
    pub iph: i32,
    pub warn_ion: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GetorbInput {
    pub nz: f64,
    pub ihole: i32,
    pub xionin: f64,
    pub iunf: i32,
    pub iph: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GetorbOutput {
    pub norb: usize,
    pub norbsc: usize,
    pub iorb: [i32; IORB_CAPACITY],
    pub iholep: i32,
    pub nq: [i32; ORBITAL_CAPACITY],
    pub kap: [i32; ORBITAL_CAPACITY],
    pub xnel: [f64; ORBITAL_CAPACITY],
    pub xnval: [f64; ORBITAL_CAPACITY],
    pub xmag: [f64; ORBITAL_CAPACITY],
}

pub type GetorbFn = dyn Fn(&GetorbInput) -> GetorbOutput;

#[derive(Debug, Clone, PartialEq)]
pub struct InmuatOutput {
    pub testy: f64,
    pub rap: [f64; 2],
    pub teste: f64,
    pub ndor: usize,
    pub nes: i32,
    pub nuc: i32,
    pub idim: usize,
    pub norb: usize,
    pub norbsc: usize,
    pub ipl: i32,
    pub iorb: [i32; IORB_CAPACITY],
    pub iholep: i32,
    pub nq: [i32; ORBITAL_CAPACITY],
    pub nq2: [i32; ORBITAL_CAPACITY],
    pub kap: [i32; ORBITAL_CAPACITY],
    pub xnel: [f64; ORBITAL_CAPACITY],
    pub xnval: [f64; ORBITAL_CAPACITY],
    pub xmag: [f64; ORBITAL_CAPACITY],
    pub en: [f64; ORBITAL_CAPACITY],
    pub scc: [f64; ORBITAL_CAPACITY],
    pub nmax: [usize; ORBITAL_CAPACITY],
    pub nre: [i32; ORBITAL_CAPACITY],
    pub eps: [f64; EPS_CAPACITY],
    pub warning: Option<String>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum InmuatError {
    #[error("getorb returned norb={norb}, but maximum is {max}")]
    InvalidOrbitalCount { norb: usize, max: usize },
    #[error("electron count mismatch: expected {expected:.6}, got {actual:.6}")]
    ElectronCountMismatch { expected: f64, actual: f64 },
    #[error("kappa out of range at orbital {index}: kappa={kappa}, nq={nq}")]
    KappaOutOfRange { index: usize, kappa: i32, nq: i32 },
}

pub fn inmuat(input: InmuatInput, getorb: &GetorbFn) -> Result<InmuatOutput, InmuatError> {
    let getorb_output = getorb(&GetorbInput {
        nz: input.nz,
        ihole: input.ihole,
        xionin: input.xionin,
        iunf: input.iunf,
        iph: input.iph,
    });

    let norb = getorb_output.norb;
    if norb > ORBITAL_CAPACITY {
        return Err(InmuatError::InvalidOrbitalCount {
            norb,
            max: ORBITAL_CAPACITY,
        });
    }

    let mut en = [0.0_f64; ORBITAL_CAPACITY];
    let mut xmag = [0.0_f64; ORBITAL_CAPACITY];
    let mut xnval = [0.0_f64; ORBITAL_CAPACITY];
    en[..norb].fill(0.0);
    xmag[..norb].copy_from_slice(&getorb_output.xmag[..norb]);
    xnval[..norb].copy_from_slice(&getorb_output.xnval[..norb]);

    let mut xk = 0.0_f64;
    let mut i = 0usize;
    while i < norb {
        xk += getorb_output.xnel[i];
        i += 1;
    }

    let expected = input.nz - input.xionin;
    let mut warning = None;
    if (expected - xk).abs() > 0.001 {
        if input.warn_ion {
            warning = Some("Warning: check number of electrons in getorb.f".to_string());
        } else {
            return Err(InmuatError::ElectronCountMismatch {
                expected,
                actual: xk,
            });
        }
    }

    let mut nmax = [0usize; ORBITAL_CAPACITY];
    let mut scc = [0.0_f64; ORBITAL_CAPACITY];
    let mut nre = [0i32; ORBITAL_CAPACITY];
    let mut eps = [0.0_f64; EPS_CAPACITY];
    eps.fill(0.0);

    let mut ipl = 0_i32;
    let idim = if DEFAULT_IDIM.is_multiple_of(2) {
        DEFAULT_IDIM - 1
    } else {
        DEFAULT_IDIM
    };

    let mut iorb_idx = 0usize;
    while iorb_idx < norb {
        nre[iorb_idx] = -1;

        let kappa = getorb_output.kap[iorb_idx];
        let mut llq = kappa.abs();
        let l = llq + llq;
        if kappa < 0 {
            llq -= 1;
        }

        let nq = getorb_output.nq[iorb_idx];
        if llq < 0 || llq >= nq || llq > 4 {
            return Err(InmuatError::KappaOutOfRange {
                index: iorb_idx + 1,
                kappa,
                nq,
            });
        }

        nmax[iorb_idx] = idim;
        scc[iorb_idx] = 0.3;
        if getorb_output.xnel[iorb_idx] < l as f64 {
            nre[iorb_idx] = 1;
        }
        if getorb_output.xnel[iorb_idx] < 0.5 {
            scc[iorb_idx] = 1.0;
        }

        let mut j = 0usize;
        while j < iorb_idx {
            if getorb_output.kap[j] == kappa && (nre[j] > 0 || nre[iorb_idx] > 0) {
                ipl += 1;
            }
            j += 1;
        }

        iorb_idx += 1;
    }

    let nq2 = getorb_output.nq;

    Ok(InmuatOutput {
        testy: 1.0e-5,
        rap: [100.0, 10.0],
        teste: 5.0e-6,
        ndor: DEFAULT_NDOR,
        nes: DEFAULT_NES,
        nuc: DEFAULT_NUC,
        idim,
        norb,
        norbsc: norb,
        ipl,
        iorb: getorb_output.iorb,
        iholep: getorb_output.iholep,
        nq: getorb_output.nq,
        nq2,
        kap: getorb_output.kap,
        xnel: getorb_output.xnel,
        xnval,
        xmag,
        en,
        scc,
        nmax,
        nre,
        eps,
        warning,
    })
}

#[cfg(test)]
mod tests {
    use super::{EPS_CAPACITY, GetorbOutput, InmuatError, InmuatInput, ORBITAL_CAPACITY, inmuat};

    fn base_getorb_output() -> GetorbOutput {
        let mut nq = [0_i32; ORBITAL_CAPACITY];
        let mut kap = [0_i32; ORBITAL_CAPACITY];
        let mut xnel = [0.0_f64; ORBITAL_CAPACITY];
        let mut xnval = [0.0_f64; ORBITAL_CAPACITY];
        let mut xmag = [0.0_f64; ORBITAL_CAPACITY];

        nq[0] = 2;
        nq[1] = 2;
        kap[0] = 1;
        kap[1] = -1;
        xnel[0] = 2.0;
        xnel[1] = 1.0;
        xnval[0] = 0.0;
        xnval[1] = 1.0;
        xmag[0] = 0.5;
        xmag[1] = 1.5;

        GetorbOutput {
            norb: 2,
            norbsc: 2,
            iorb: [0; 10],
            iholep: 1,
            nq,
            kap,
            xnel,
            xnval,
            xmag,
        }
    }

    #[test]
    fn inmuat_initializes_atomic_state_and_shell_flags() {
        let output = inmuat(
            InmuatInput {
                nz: 3.0,
                ihole: 1,
                xionin: 0.0,
                iunf: 0,
                iph: 0,
                warn_ion: false,
            },
            &|_| base_getorb_output(),
        )
        .expect("inmuat should succeed");

        assert_eq!(output.norb, 2);
        assert_eq!(output.norbsc, 2);
        assert_eq!(output.ndor, 10);
        assert_eq!(output.nes, 50);
        assert_eq!(output.nuc, 11);
        assert_eq!(output.idim, 251);
        assert_eq!(output.ipl, 0);
        assert_eq!(output.nmax[0], 251);
        assert_eq!(output.nre[0], -1);
        assert_eq!(output.nre[1], 1);
        assert_eq!(output.scc[0], 0.3);
        assert_eq!(output.scc[1], 0.3);
        assert!(
            output
                .eps
                .iter()
                .take(EPS_CAPACITY)
                .all(|value| *value == 0.0)
        );
    }

    #[test]
    fn inmuat_can_warn_on_electron_count_mismatch() {
        let output = inmuat(
            InmuatInput {
                nz: 5.0,
                ihole: 1,
                xionin: 0.0,
                iunf: 0,
                iph: 0,
                warn_ion: true,
            },
            &|_| base_getorb_output(),
        )
        .expect("warn_ion=true should not fail");

        assert!(output.warning.is_some());
    }

    #[test]
    fn inmuat_errors_when_electron_count_mismatch_is_fatal() {
        let error = inmuat(
            InmuatInput {
                nz: 5.0,
                ihole: 1,
                xionin: 0.0,
                iunf: 0,
                iph: 0,
                warn_ion: false,
            },
            &|_| base_getorb_output(),
        )
        .expect_err("warn_ion=false should fail on mismatch");

        assert!(matches!(error, InmuatError::ElectronCountMismatch { .. }));
    }

    #[test]
    fn inmuat_validates_kappa_quantum_number_range() {
        let error = inmuat(
            InmuatInput {
                nz: 3.0,
                ihole: 1,
                xionin: 0.0,
                iunf: 0,
                iph: 0,
                warn_ion: false,
            },
            &|_| {
                let mut output = base_getorb_output();
                output.kap[0] = 5;
                output.nq[0] = 5;
                output
            },
        )
        .expect_err("invalid kappa should fail");

        assert!(matches!(error, InmuatError::KappaOutOfRange { .. }));
    }
}
