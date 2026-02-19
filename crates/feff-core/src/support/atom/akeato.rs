#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum AkeatoError {
    #[error("orbital index must be >= 1, got {0}")]
    InvalidOrbitalIndex(usize),
    #[error("k must be >= 0, got {0}")]
    NegativeK(i32),
    #[error("k must be even, got {0}")]
    OddK(i32),
    #[error("table entry is missing for (i={i}, j={j}, k/2={k_half})")]
    MissingEntry { i: usize, j: usize, k_half: usize },
}

pub fn akeato(i: usize, j: usize, k: i32, afgk: &[Vec<Vec<f64>>]) -> Result<f64, AkeatoError> {
    let k_half = k_half(k)?;
    let (row, col) = if i <= j { (i, j) } else { (j, i) };
    coefficient(afgk, row, col, k_half)
}

pub fn bkeato(i: usize, j: usize, k: i32, afgk: &[Vec<Vec<f64>>]) -> Result<f64, AkeatoError> {
    if i == j {
        return Ok(0.0);
    }
    let k_half = k_half(k)?;
    let (row, col) = if i < j { (j, i) } else { (i, j) };
    coefficient(afgk, row, col, k_half)
}

fn k_half(k: i32) -> Result<usize, AkeatoError> {
    if k < 0 {
        return Err(AkeatoError::NegativeK(k));
    }
    if k % 2 != 0 {
        return Err(AkeatoError::OddK(k));
    }
    Ok((k / 2) as usize)
}

fn coefficient(
    afgk: &[Vec<Vec<f64>>],
    i: usize,
    j: usize,
    k_half: usize,
) -> Result<f64, AkeatoError> {
    if i == 0 {
        return Err(AkeatoError::InvalidOrbitalIndex(i));
    }
    if j == 0 {
        return Err(AkeatoError::InvalidOrbitalIndex(j));
    }
    afgk.get(i - 1)
        .and_then(|row| row.get(j - 1))
        .and_then(|k_row| k_row.get(k_half))
        .copied()
        .ok_or(AkeatoError::MissingEntry { i, j, k_half })
}

#[cfg(test)]
mod tests {
    use super::{AkeatoError, akeato, bkeato};

    fn make_table() -> Vec<Vec<Vec<f64>>> {
        let mut table = vec![vec![vec![0.0; 3]; 3]; 3];
        for (i, table_i) in table.iter_mut().enumerate() {
            for (j, table_j) in table_i.iter_mut().enumerate() {
                for (k_half, coeff) in table_j.iter_mut().enumerate() {
                    *coeff = ((i + 1) as f64) * 100.0 + ((j + 1) as f64) * 10.0 + k_half as f64;
                }
            }
        }
        table
    }

    #[test]
    fn akeato_uses_min_max_orbital_order() {
        let table = make_table();
        let forward = akeato(1, 3, 4, &table).expect("valid angular lookup should work");
        let reverse = akeato(3, 1, 4, &table).expect("reversed order should map to same term");
        assert_eq!(forward, 132.0);
        assert_eq!(reverse, 132.0);
    }

    #[test]
    fn bkeato_uses_exchange_order_and_zero_on_diagonal() {
        let table = make_table();
        let exchange = bkeato(1, 3, 4, &table).expect("exchange lookup should work");
        assert_eq!(exchange, 312.0);
        assert_eq!(
            bkeato(2, 2, 2, &table).expect("diagonal exchange is zero"),
            0.0
        );
    }

    #[test]
    fn odd_k_is_rejected() {
        let table = make_table();
        let error = akeato(1, 1, 3, &table).expect_err("odd k should fail");
        assert_eq!(error, AkeatoError::OddK(3));
    }
}
