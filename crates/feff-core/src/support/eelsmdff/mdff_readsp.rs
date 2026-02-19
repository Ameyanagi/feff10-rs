use super::mdff_concat::mdff_concat;
use super::mdff_eels::SigmaTensorRow;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdffInputKind {
    Xmu,
    OpconsKk,
}

impl MdffInputKind {
    fn min_column_count(self) -> usize {
        match self {
            MdffInputKind::Xmu => 6,
            MdffInputKind::OpconsKk => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MdffReadspConfig {
    pub ipmin: usize,
    pub ipmax: usize,
    pub ipstep: usize,
    pub average: bool,
    pub cross_terms: bool,
    pub spcol: usize,
    pub input_kind: MdffInputKind,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum MdffReadspError {
    #[error("ipstep must be at least 1")]
    InvalidIpStep,
    #[error("spcol must be at least 1")]
    InvalidSpectrumColumn,
    #[error("missing spectrum input for ip={ip}")]
    MissingSpectrum { ip: usize },
    #[error("spectrum input for ip={ip} has no numeric rows")]
    EmptySpectrum { ip: usize },
    #[error(
        "spectrum input for ip={ip} row {row} has {actual} columns; expected at least {expected}"
    )]
    ColumnCountMismatch {
        ip: usize,
        row: usize,
        expected: usize,
        actual: usize,
    },
    #[error("spectrum row count mismatch for ip={ip}; expected {expected}, got {actual}")]
    RowCountMismatch {
        ip: usize,
        expected: usize,
        actual: usize,
    },
    #[error("energy-grid mismatch for ip={ip} row {row}; expected {expected}, got {actual}")]
    EnergyGridMismatch {
        ip: usize,
        row: usize,
        expected: f64,
        actual: f64,
    },
    #[error("orientation-sensitive mode requires ipmin=1 and ipmax=9")]
    InvalidOrientationRange,
    #[error("average mode requires either [1..9] or [10..10]")]
    InvalidAverageRange,
    #[error("cross-term mode requires ipstep=1, got {ipstep}")]
    MissingCrossTermInputs { ipstep: usize },
    #[error("invalid ip value {ip}; expected 1..10")]
    InvalidIp { ip: usize },
}

pub fn mdff_spectrum_filename(prefix: &str, ip: usize) -> Result<String, MdffReadspError> {
    let suffix = match ip {
        1 => ".dat  ".to_string(),
        10 => "10.dat".to_string(),
        2..=9 => format!("0{ip}.dat"),
        _ => return Err(MdffReadspError::InvalidIp { ip }),
    };

    Ok(mdff_concat(prefix, &suffix).0)
}

pub fn mdff_readsp(
    sources_by_ip: &BTreeMap<usize, &str>,
    config: MdffReadspConfig,
) -> Result<Vec<SigmaTensorRow>, MdffReadspError> {
    if config.ipstep == 0 {
        return Err(MdffReadspError::InvalidIpStep);
    }
    if config.spcol == 0 {
        return Err(MdffReadspError::InvalidSpectrumColumn);
    }

    let mut rows_by_ip: BTreeMap<usize, Vec<(f64, f64)>> = BTreeMap::new();
    for ip in (config.ipmin..=config.ipmax).step_by(config.ipstep) {
        let source = sources_by_ip
            .get(&ip)
            .copied()
            .ok_or(MdffReadspError::MissingSpectrum { ip })?;
        let rows = parse_spectrum_rows(source, ip, config.input_kind, config.spcol)?;
        rows_by_ip.insert(ip, rows);
    }

    let first_key = rows_by_ip
        .keys()
        .next()
        .copied()
        .ok_or(MdffReadspError::EmptySpectrum { ip: config.ipmin })?;
    let first_rows = rows_by_ip
        .get(&first_key)
        .expect("first key must exist in parsed rows");
    let energy_grid = first_rows
        .iter()
        .map(|(energy, _)| *energy)
        .collect::<Vec<_>>();

    for (&ip, rows) in &rows_by_ip {
        if rows.len() != energy_grid.len() {
            return Err(MdffReadspError::RowCountMismatch {
                ip,
                expected: energy_grid.len(),
                actual: rows.len(),
            });
        }

        for (row_index, ((expected_energy, _), (actual_energy, _))) in
            first_rows.iter().zip(rows.iter()).enumerate()
        {
            if (expected_energy - actual_energy).abs() > 1.0e-9 {
                return Err(MdffReadspError::EnergyGridMismatch {
                    ip,
                    row: row_index,
                    expected: *expected_energy,
                    actual: *actual_energy,
                });
            }
        }
    }

    let mut tensors = vec![[0.0_f64; 9]; energy_grid.len()];

    if config.average {
        apply_average_mode(&rows_by_ip, &mut tensors, config)?;
    } else {
        apply_orientation_mode(&rows_by_ip, &mut tensors, config)?;
    }

    Ok(energy_grid
        .into_iter()
        .zip(tensors)
        .map(|(energy_loss_ev, tensor)| SigmaTensorRow::from_flat(energy_loss_ev, tensor))
        .collect())
}

fn parse_spectrum_rows(
    source: &str,
    ip: usize,
    input_kind: MdffInputKind,
    spcol: usize,
) -> Result<Vec<(f64, f64)>, MdffReadspError> {
    let mut rows = Vec::new();
    let min_columns = input_kind.min_column_count().max(spcol);

    for (line_index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }

        let columns = parse_numeric_tokens(trimmed);
        if columns.is_empty() {
            continue;
        }
        if columns.len() < min_columns {
            return Err(MdffReadspError::ColumnCountMismatch {
                ip,
                row: line_index,
                expected: min_columns,
                actual: columns.len(),
            });
        }

        rows.push((columns[0], columns[spcol - 1]));
    }

    if rows.is_empty() {
        return Err(MdffReadspError::EmptySpectrum { ip });
    }

    Ok(rows)
}

fn parse_numeric_tokens(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(|token| {
            token
                .trim()
                .trim_end_matches([',', ';', ':'])
                .replace(['D', 'd'], "E")
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
        })
        .collect()
}

fn apply_orientation_mode(
    rows_by_ip: &BTreeMap<usize, Vec<(f64, f64)>>,
    tensors: &mut [[f64; 9]],
    config: MdffReadspConfig,
) -> Result<(), MdffReadspError> {
    if config.ipmin != 1 || config.ipmax != 9 {
        return Err(MdffReadspError::InvalidOrientationRange);
    }

    let mut ip_step_local = config.ipstep;
    if config.cross_terms {
        if ip_step_local != 1 {
            return Err(MdffReadspError::MissingCrossTermInputs {
                ipstep: ip_step_local,
            });
        }
    } else if ip_step_local == 1 {
        ip_step_local = 4;
    }

    for ip in (config.ipmin..=config.ipmax).step_by(ip_step_local) {
        assign_ip_column(rows_by_ip, tensors, ip)?;
    }

    Ok(())
}

fn apply_average_mode(
    rows_by_ip: &BTreeMap<usize, Vec<(f64, f64)>>,
    tensors: &mut [[f64; 9]],
    config: MdffReadspConfig,
) -> Result<(), MdffReadspError> {
    if config.ipmin == 10 && config.ipmax == 10 {
        let rows = rows_by_ip
            .get(&10)
            .ok_or(MdffReadspError::MissingSpectrum { ip: 10 })?;
        for (row_index, (_, value)) in rows.iter().enumerate() {
            tensors[row_index][0] = *value;
            tensors[row_index][4] = *value;
            tensors[row_index][8] = *value;
        }
        return Ok(());
    }

    if config.ipmin == 1 && config.ipmax == 9 {
        let xx = rows_by_ip
            .get(&1)
            .ok_or(MdffReadspError::MissingSpectrum { ip: 1 })?;
        let yy = rows_by_ip
            .get(&5)
            .ok_or(MdffReadspError::MissingSpectrum { ip: 5 })?;
        let zz = rows_by_ip
            .get(&9)
            .ok_or(MdffReadspError::MissingSpectrum { ip: 9 })?;

        for row_index in 0..tensors.len() {
            let value = (xx[row_index].1 + yy[row_index].1 + zz[row_index].1) / 3.0;
            tensors[row_index][0] = value;
            tensors[row_index][4] = value;
            tensors[row_index][8] = value;
        }

        return Ok(());
    }

    Err(MdffReadspError::InvalidAverageRange)
}

fn assign_ip_column(
    rows_by_ip: &BTreeMap<usize, Vec<(f64, f64)>>,
    tensors: &mut [[f64; 9]],
    ip: usize,
) -> Result<(), MdffReadspError> {
    if !(1..=9).contains(&ip) {
        return Ok(());
    }

    let column_index = ip - 1;
    let rows = rows_by_ip
        .get(&ip)
        .ok_or(MdffReadspError::MissingSpectrum { ip })?;

    for (row_index, (_, value)) in rows.iter().enumerate() {
        tensors[row_index][column_index] = *value;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        MdffInputKind, MdffReadspConfig, MdffReadspError, mdff_readsp, mdff_spectrum_filename,
    };
    use std::collections::BTreeMap;

    fn mk_source(ip: usize, scale: f64) -> String {
        let mut body = String::from("# omega e k mu mu0 chi\n");
        body.push_str(&format!("10.0 0.0 0.0 {} 0.0 0.0\n", scale * ip as f64));
        body.push_str(&format!(
            "12.0 0.0 0.0 {} 0.0 0.0\n",
            scale * ip as f64 + 1.0
        ));
        body
    }

    fn all_ip_sources() -> BTreeMap<usize, String> {
        (1..=9).map(|ip| (ip, mk_source(ip, 1.0))).collect()
    }

    #[test]
    fn filename_builder_matches_legacy_ip_rules() {
        assert_eq!(
            mdff_spectrum_filename("xmu", 1).expect("ip=1 filename"),
            "xmu.dat  "
        );
        assert_eq!(
            mdff_spectrum_filename("xmu", 4).expect("ip=4 filename"),
            "xmu04.dat"
        );
        assert_eq!(
            mdff_spectrum_filename("xmu", 10).expect("ip=10 filename"),
            "xmu10.dat"
        );
    }

    #[test]
    fn orientation_without_cross_terms_keeps_diagonal_components() {
        let owned_sources = all_ip_sources();
        let borrowed_sources = owned_sources
            .iter()
            .map(|(&ip, source)| (ip, source.as_str()))
            .collect::<BTreeMap<_, _>>();

        let rows = mdff_readsp(
            &borrowed_sources,
            MdffReadspConfig {
                ipmin: 1,
                ipmax: 9,
                ipstep: 1,
                average: false,
                cross_terms: false,
                spcol: 4,
                input_kind: MdffInputKind::Xmu,
            },
        )
        .expect("readsp should succeed");

        assert_eq!(rows.len(), 2);
        let tensor = rows[0].flatten();
        assert_eq!(tensor[0], 1.0);
        assert_eq!(tensor[4], 5.0);
        assert_eq!(tensor[8], 9.0);
        assert_eq!(tensor[1], 0.0);
        assert_eq!(tensor[3], 0.0);
    }

    #[test]
    fn orientation_with_cross_terms_uses_all_tensor_channels() {
        let owned_sources = all_ip_sources();
        let borrowed_sources = owned_sources
            .iter()
            .map(|(&ip, source)| (ip, source.as_str()))
            .collect::<BTreeMap<_, _>>();

        let rows = mdff_readsp(
            &borrowed_sources,
            MdffReadspConfig {
                ipmin: 1,
                ipmax: 9,
                ipstep: 1,
                average: false,
                cross_terms: true,
                spcol: 4,
                input_kind: MdffInputKind::Xmu,
            },
        )
        .expect("readsp should succeed");

        let tensor = rows[0].flatten();
        assert_eq!(tensor, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0]);
    }

    #[test]
    fn average_mode_averages_xx_yy_zz_channels() {
        let owned_sources = all_ip_sources();
        let borrowed_sources = owned_sources
            .iter()
            .map(|(&ip, source)| (ip, source.as_str()))
            .collect::<BTreeMap<_, _>>();

        let rows = mdff_readsp(
            &borrowed_sources,
            MdffReadspConfig {
                ipmin: 1,
                ipmax: 9,
                ipstep: 1,
                average: true,
                cross_terms: false,
                spcol: 4,
                input_kind: MdffInputKind::Xmu,
            },
        )
        .expect("readsp should succeed");

        let tensor = rows[0].flatten();
        let expected = (1.0 + 5.0 + 9.0) / 3.0;
        assert_eq!(tensor[0], expected);
        assert_eq!(tensor[4], expected);
        assert_eq!(tensor[8], expected);
        assert_eq!(tensor[1], 0.0);
        assert_eq!(tensor[2], 0.0);
    }

    #[test]
    fn cross_term_mode_requires_dense_ip_grid() {
        let owned_sources = all_ip_sources();
        let borrowed_sources = owned_sources
            .iter()
            .map(|(&ip, source)| (ip, source.as_str()))
            .collect::<BTreeMap<_, _>>();

        let error = mdff_readsp(
            &borrowed_sources,
            MdffReadspConfig {
                ipmin: 1,
                ipmax: 9,
                ipstep: 4,
                average: false,
                cross_terms: true,
                spcol: 4,
                input_kind: MdffInputKind::Xmu,
            },
        )
        .expect_err("cross terms should require ipstep=1");

        assert_eq!(error, MdffReadspError::MissingCrossTermInputs { ipstep: 4 });
    }
}
