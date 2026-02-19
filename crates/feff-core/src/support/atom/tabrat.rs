pub type DsordfFn = dyn FnMut(i32, i32, i32, i32, f64) -> f64;

const RYD: f64 = 13.605_698;
const HART: f64 = 2.0 * RYD;
const TTIRE: [&str; 9] = ["s ", "p*", "p ", "d*", "d ", "f*", "f ", "g*", "g "];

#[derive(Debug, Clone)]
pub struct TabratInput<'a> {
    pub norb: usize,
    pub nq: &'a [i32],
    pub kap: &'a [i32],
    pub xnel: &'a [f64],
    pub en: &'a [f64],
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrbitalTabulation {
    pub orbital_index: usize,
    pub n: i32,
    pub title: String,
    pub electrons: f64,
    pub binding_energy_hartree: f64,
    pub radial_moments: Vec<(i32, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OverlapEntry {
    pub i: usize,
    pub j: usize,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TabratOutput {
    pub mbi: [i32; 7],
    pub orbitals: Vec<OrbitalTabulation>,
    pub overlaps: Vec<OverlapEntry>,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum TabratError {
    #[error("norb must be >= 1")]
    InvalidNorb,
    #[error("input length mismatch for {name}: need at least {need}, got {got}")]
    LengthMismatch {
        name: &'static str,
        need: usize,
        got: usize,
    },
    #[error("unsupported kappa={kappa} at orbital {index}")]
    UnsupportedKappa { index: usize, kappa: i32 },
}

pub fn tabrat(input: &TabratInput<'_>, dsordf: &mut DsordfFn) -> Result<TabratOutput, TabratError> {
    if input.norb == 0 {
        return Err(TabratError::InvalidNorb);
    }
    ensure_len("nq", input.nq.len(), input.norb)?;
    ensure_len("kap", input.kap.len(), input.norb)?;
    ensure_len("xnel", input.xnel.len(), input.norb)?;
    ensure_len("en", input.en.len(), input.norb)?;

    let mut mbi = [0_i32; 7];
    let mut idx = 0usize;
    while idx < 7 {
        let i = idx as i32 + 2;
        mbi[idx] = 8 - i - i / 3 - i / 4 + i / 8;
        idx += 1;
    }

    let mut orbitals = Vec::with_capacity(input.norb);
    let mut i = 0usize;
    while i < input.norb {
        let title = kappa_label(input.kap[i], i + 1)?.to_string();
        let llq = input.kap[i].abs() - 1;
        let j_limit = if llq <= 0 { 7 } else { 8 };

        let mut moments = Vec::with_capacity(j_limit - 1);
        let mut k = 2usize;
        while k <= j_limit {
            let power = mbi[k - 2];
            let value = dsordf((i + 1) as i32, (i + 1) as i32, power, 1, 0.0);
            moments.push((power, value));
            k += 1;
        }

        orbitals.push(OrbitalTabulation {
            orbital_index: i + 1,
            n: input.nq[i],
            title,
            electrons: input.xnel[i],
            binding_energy_hartree: -input.en[i] * HART,
            radial_moments: moments,
        });

        i += 1;
    }

    let mut overlaps = Vec::new();
    let mut i_outer = 0usize;
    while i_outer + 1 < input.norb {
        let mut j = i_outer + 1;
        while j < input.norb {
            if input.kap[j] == input.kap[i_outer] {
                overlaps.push(OverlapEntry {
                    i: i_outer + 1,
                    j: j + 1,
                    value: dsordf((i_outer + 1) as i32, (j + 1) as i32, 0, 1, 0.0),
                });
            }
            j += 1;
        }
        i_outer += 1;
    }

    Ok(TabratOutput {
        mbi,
        orbitals,
        overlaps,
    })
}

fn kappa_label(kappa: i32, index: usize) -> Result<&'static str, TabratError> {
    let position = if kappa > 0 { 2 * kappa } else { -2 * kappa - 1 };

    if !(1..=TTIRE.len() as i32).contains(&position) {
        return Err(TabratError::UnsupportedKappa { index, kappa });
    }

    Ok(TTIRE[(position - 1) as usize])
}

fn ensure_len(name: &'static str, got: usize, need: usize) -> Result<(), TabratError> {
    if got < need {
        return Err(TabratError::LengthMismatch { name, need, got });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{TabratError, TabratInput, tabrat};

    #[test]
    fn tabrat_builds_titles_moments_and_overlaps() {
        let mut dsordf = |i: i32, j: i32, n: i32, _: i32, _: f64| i as f64 + j as f64 + n as f64;

        let output = tabrat(
            &TabratInput {
                norb: 3,
                nq: &[1, 2, 2],
                kap: &[1, 1, -1],
                xnel: &[2.0, 1.0, 1.0],
                en: &[-0.5, -0.2, -0.1],
            },
            &mut dsordf,
        )
        .expect("tabrat should succeed");

        assert_eq!(output.orbitals[0].title, "p*");
        assert_eq!(output.orbitals[2].title, "s ");
        assert_eq!(output.orbitals[0].radial_moments.len(), 6);
        assert_eq!(output.overlaps.len(), 1);
        assert_eq!(output.overlaps[0].i, 1);
        assert_eq!(output.overlaps[0].j, 2);
    }

    #[test]
    fn tabrat_rejects_unsupported_kappa_values() {
        let mut dsordf = |_: i32, _: i32, _: i32, _: i32, _: f64| 0.0;
        let error = tabrat(
            &TabratInput {
                norb: 1,
                nq: &[1],
                kap: &[9],
                xnel: &[1.0],
                en: &[-1.0],
            },
            &mut dsordf,
        )
        .expect_err("invalid kappa should fail");

        assert_eq!(error, TabratError::UnsupportedKappa { index: 1, kappa: 9 });
    }
}
