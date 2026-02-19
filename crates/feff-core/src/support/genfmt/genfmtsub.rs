use super::m_genfmt::{GeneratedPathRecord, GenfmtArtifacts};

#[derive(Debug, Clone, PartialEq)]
pub struct GenfmtSubConfig {
    pub version_tag: String,
    pub critcw: f64,
    pub iorder: i32,
    pub include_nstar: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenfmtPathInput {
    pub path_index: usize,
    pub nleg: usize,
    pub degeneracy: f64,
    pub reff: f64,
    pub amplitude: f64,
}

pub fn generated_path_records(paths: &[GenfmtPathInput], critcw: f64) -> Vec<GeneratedPathRecord> {
    let amplitude_max = paths
        .iter()
        .map(|path| path.amplitude.abs())
        .fold(0.0_f64, f64::max);

    paths
        .iter()
        .map(|path| {
            let ratio = if amplitude_max <= f64::EPSILON {
                0.0
            } else {
                path.amplitude.abs() * 100.0 / amplitude_max
            };
            GeneratedPathRecord {
                path_index: path.path_index,
                nleg: path.nleg,
                degeneracy: path.degeneracy,
                reff: path.reff,
                cw_amplitude_ratio: ratio.max(critcw).min(100.0),
            }
        })
        .collect()
}

pub fn genfmt(config: &GenfmtSubConfig, paths: &[GenfmtPathInput]) -> GenfmtArtifacts {
    let records = generated_path_records(paths, config.critcw.max(0.0));
    let mut artifacts = GenfmtArtifacts {
        feff_header_lines: vec![
            format!("#_feff.bin v03: {}", config.version_tag),
            format!(
                "#= iorder={} critcw={:.2} paths={}",
                config.iorder,
                config.critcw,
                records.len()
            ),
        ],
        list_header_lines: vec![
            " -----------------------------------------------------------------------".to_string(),
            "  pathindex     sig2   amp ratio       deg    nlegs  r effective".to_string(),
        ],
        list_rows: Vec::with_capacity(records.len()),
        nstar_rows: Vec::new(),
    };

    for record in &records {
        let sig2 = 1.0 / (1.0 + record.reff.max(0.0));
        artifacts.list_rows.push(format!(
            "{:>10} {:>8.4} {:>10.3} {:>10.3} {:>8} {:>11.4}",
            record.path_index,
            sig2,
            record.cw_amplitude_ratio,
            record.degeneracy,
            record.nleg,
            record.reff,
        ));

        if config.include_nstar {
            let nstar = record.degeneracy.max(0.0) * record.reff.max(0.0).sqrt();
            artifacts
                .nstar_rows
                .push(format!("{:>6} {:>10.3}", record.path_index, nstar));
        }
    }

    artifacts
}

#[cfg(test)]
mod tests {
    use super::{GenfmtPathInput, GenfmtSubConfig, generated_path_records, genfmt};

    #[test]
    fn generated_path_records_normalize_against_max_amplitude() {
        let records = generated_path_records(
            &[
                GenfmtPathInput {
                    path_index: 1,
                    nleg: 2,
                    degeneracy: 4.0,
                    reff: 2.4,
                    amplitude: 0.5,
                },
                GenfmtPathInput {
                    path_index: 2,
                    nleg: 3,
                    degeneracy: 2.0,
                    reff: 3.1,
                    amplitude: 1.0,
                },
            ],
            0.0,
        );

        assert!((records[0].cw_amplitude_ratio - 50.0).abs() < 1.0e-12);
        assert!((records[1].cw_amplitude_ratio - 100.0).abs() < 1.0e-12);
    }

    #[test]
    fn genfmt_builds_deterministic_list_and_header_artifacts() {
        let artifacts = genfmt(
            &GenfmtSubConfig {
                version_tag: "10.1.0".to_string(),
                critcw: 5.0,
                iorder: 2,
                include_nstar: false,
            },
            &[
                GenfmtPathInput {
                    path_index: 3,
                    nleg: 4,
                    degeneracy: 8.0,
                    reff: 4.2,
                    amplitude: 0.8,
                },
                GenfmtPathInput {
                    path_index: 4,
                    nleg: 3,
                    degeneracy: 6.0,
                    reff: 3.6,
                    amplitude: 0.2,
                },
            ],
        );

        assert_eq!(artifacts.feff_header_lines[0], "#_feff.bin v03: 10.1.0");
        assert_eq!(artifacts.list_rows.len(), 2);
        assert!(artifacts.list_rows[0].contains("3"));
        assert!(artifacts.list_rows[0].contains("100.000"));
    }

    #[test]
    fn genfmt_populates_nstar_artifact_when_requested() {
        let artifacts = genfmt(
            &GenfmtSubConfig {
                version_tag: "10.1.0".to_string(),
                critcw: 0.0,
                iorder: 2,
                include_nstar: true,
            },
            &[GenfmtPathInput {
                path_index: 1,
                nleg: 2,
                degeneracy: 4.0,
                reff: 2.25,
                amplitude: 1.0,
            }],
        );

        assert_eq!(artifacts.nstar_rows.len(), 1);
        assert!(artifacts.nstar_rows[0].contains("6.000"));
    }
}
