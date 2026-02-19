use super::nucmass::{NucmassError, nucmass};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NucdevInput {
    pub dz: f64,
    pub hx: f64,
    pub nuc: i32,
    pub np: usize,
    pub ndor: usize,
    pub dr1: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NucdevOutput {
    pub av: Vec<f64>,
    pub dr: Vec<f64>,
    pub dv: Vec<f64>,
    pub nuc: i32,
    pub dr1: f64,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum NucdevError {
    #[error("nuclear charge dz must be > 0, got {0}")]
    InvalidCharge(f64),
    #[error("np must be >= 1")]
    InvalidGridSize,
    #[error("ndor must be >= 5, got {0}")]
    InvalidSeriesOrder(usize),
    #[error("nuclear radius index nuc={nuc} is outside 1..={np}")]
    InvalidNuclearIndex { nuc: i32, np: usize },
    #[error("dr1 too small for requested nuclear radius")]
    Dr1TooSmall,
    #[error(transparent)]
    Nucmass(#[from] NucmassError),
}

pub fn nucdev(input: NucdevInput) -> Result<NucdevOutput, NucdevError> {
    if input.dz <= 0.0 {
        return Err(NucdevError::InvalidCharge(input.dz));
    }
    if input.np == 0 {
        return Err(NucdevError::InvalidGridSize);
    }
    if input.ndor < 5 {
        return Err(NucdevError::InvalidSeriesOrder(input.ndor));
    }

    let mut nuc = input.nuc;
    let mut dr1 = input.dr1;
    let mut a = 0.0_f64;

    if nuc < 0 {
        let iz = input.dz as i32;
        a = nucmass(iz)?;
        nuc = -nuc;
    }

    if a <= 1.0e-1 {
        nuc = 1;
    } else {
        a = input.dz * a.powf(1.0 / 3.0) * 2.2677e-5;
        let mut b = a / (input.hx * (nuc - 1) as f64).exp();
        if b <= dr1 {
            dr1 = b;
        } else {
            b = (a / dr1).ln() / input.hx;
            nuc = 3 + 2 * ((b / 2.0) as i32);
            if nuc as usize >= input.np {
                return Err(NucdevError::Dr1TooSmall);
            }
            dr1 = a * (-(nuc - 1) as f64 * input.hx).exp();
        }
    }

    if nuc <= 0 || nuc as usize > input.np {
        return Err(NucdevError::InvalidNuclearIndex { nuc, np: input.np });
    }

    let mut dr = vec![0.0_f64; input.np];
    let mut dv = vec![0.0_f64; input.np];
    let mut av = vec![0.0_f64; input.ndor];

    dr[0] = dr1 / input.dz;
    let mut i = 1usize;
    while i < input.np {
        dr[i] = dr[0] * (input.hx * i as f64).exp();
        i += 1;
    }

    let mut idx = 0usize;
    while idx < input.np {
        dv[idx] = -input.dz / dr[idx];
        idx += 1;
    }

    if nuc <= 1 {
        av[0] = -input.dz;
    } else {
        let nuc_idx = (nuc - 1) as usize;
        av[1] = -3.0 * input.dz / (dr[nuc_idx] + dr[nuc_idx]);
        av[3] = -av[1] / (3.0 * dr[nuc_idx] * dr[nuc_idx]);

        let mut j = 0usize;
        while j < nuc_idx {
            dv[j] = av[1] + av[3] * dr[j] * dr[j];
            j += 1;
        }
    }

    Ok(NucdevOutput {
        av,
        dr,
        dv,
        nuc,
        dr1,
    })
}

#[cfg(test)]
mod tests {
    use super::{NucdevError, NucdevInput, nucdev};

    #[test]
    fn nucdev_builds_point_charge_potential_for_default_nucleus() {
        let output = nucdev(NucdevInput {
            dz: 26.0,
            hx: 0.1,
            nuc: 1,
            np: 5,
            ndor: 5,
            dr1: 1.0e-3,
        })
        .expect("nucdev should succeed");

        assert_eq!(output.nuc, 1);
        assert!((output.av[0] + 26.0).abs() <= 1.0e-12);
        assert!((output.dv[0] + 26.0 / output.dr[0]).abs() <= 1.0e-12);
    }

    #[test]
    fn nucdev_supports_high_z_mass_lookup_mode() {
        let output = nucdev(NucdevInput {
            dz: 79.0,
            hx: 0.05,
            nuc: -11,
            np: 251,
            ndor: 6,
            dr1: 1.0e-3,
        })
        .expect("nucdev should succeed with nucmass lookup");

        assert!(output.nuc > 1);
        assert!(output.dr[0] > 0.0);
        assert!(output.dv.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn nucdev_requires_at_least_five_series_terms() {
        let error = nucdev(NucdevInput {
            dz: 1.0,
            hx: 0.1,
            nuc: 1,
            np: 3,
            ndor: 4,
            dr1: 1.0e-3,
        })
        .expect_err("ndor<5 should fail");

        assert_eq!(error, NucdevError::InvalidSeriesOrder(4));
    }
}
