use super::genfmtsub::{GenfmtPathInput, generated_path_records};
use super::m_genfmt::GenfmtArtifacts;

#[derive(Debug, Clone, PartialEq)]
pub struct GenfmtJasConfig {
    pub version_tag: String,
    pub critcw: f64,
    pub iorder: i32,
    pub q_weights: Vec<f64>,
}

pub fn genfmtjas(config: &GenfmtJasConfig, paths: &[GenfmtPathInput]) -> GenfmtArtifacts {
    let records = generated_path_records(paths, config.critcw.max(0.0));
    let q_weight_scale = normalized_q_weight(&config.q_weights);

    let mut artifacts = GenfmtArtifacts {
        feff_header_lines: vec![
            format!("#_feff.bin v03: {}", config.version_tag),
            format!(
                "#= nrixs iorder={} critcw={:.2} q_count={}",
                config.iorder,
                config.critcw,
                config.q_weights.len().max(1),
            ),
        ],
        list_header_lines: vec![
            " -----------------------------------------------------------------------------"
                .to_string(),
            "  pathindex   q-weighted ratio    deg    nlegs   r effective".to_string(),
        ],
        list_rows: Vec::with_capacity(records.len()),
        nstar_rows: Vec::new(),
    };

    for record in records {
        let q_ratio = (record.cw_amplitude_ratio * q_weight_scale).clamp(0.0, 100.0);
        artifacts.list_rows.push(format!(
            "{:>10} {:>18.3} {:>8.3} {:>8} {:>12.4}",
            record.path_index, q_ratio, record.degeneracy, record.nleg, record.reff,
        ));

        let weighted_nstar =
            record.degeneracy.max(0.0) * q_weight_scale * record.reff.max(0.0).sqrt();
        artifacts
            .nstar_rows
            .push(format!("{:>6} {:>10.3}", record.path_index, weighted_nstar,));
    }

    artifacts
}

fn normalized_q_weight(weights: &[f64]) -> f64 {
    if weights.is_empty() {
        return 1.0;
    }

    let positive_sum: f64 = weights.iter().copied().filter(|value| *value > 0.0).sum();
    if positive_sum <= f64::EPSILON {
        return 1.0;
    }

    positive_sum / weights.len() as f64
}

#[cfg(test)]
mod tests {
    use super::{GenfmtJasConfig, genfmtjas};
    use crate::support::genfmt::genfmtsub::GenfmtPathInput;

    #[test]
    fn genfmtjas_embeds_nrixs_header_fields() {
        let artifacts = genfmtjas(
            &GenfmtJasConfig {
                version_tag: "10.2.1".to_string(),
                critcw: 3.0,
                iorder: 2,
                q_weights: vec![1.0, 0.5],
            },
            &[GenfmtPathInput {
                path_index: 7,
                nleg: 4,
                degeneracy: 5.0,
                reff: 3.8,
                amplitude: 1.0,
            }],
        );

        assert!(artifacts.feff_header_lines[1].contains("nrixs"));
        assert!(artifacts.feff_header_lines[1].contains("q_count=2"));
    }

    #[test]
    fn genfmtjas_q_weights_scale_reported_ratio() {
        let paths = [GenfmtPathInput {
            path_index: 2,
            nleg: 3,
            degeneracy: 4.0,
            reff: 2.5,
            amplitude: 1.0,
        }];

        let heavy = genfmtjas(
            &GenfmtJasConfig {
                version_tag: "10.0".to_string(),
                critcw: 0.0,
                iorder: 2,
                q_weights: vec![1.0, 1.0],
            },
            &paths,
        );

        let light = genfmtjas(
            &GenfmtJasConfig {
                version_tag: "10.0".to_string(),
                critcw: 0.0,
                iorder: 2,
                q_weights: vec![0.2, 0.2],
            },
            &paths,
        );

        let heavy_ratio = heavy
            .list_rows
            .first()
            .expect("list row should exist")
            .split_whitespace()
            .nth(1)
            .expect("ratio column should be present")
            .parse::<f64>()
            .expect("ratio should parse");
        let light_ratio = light
            .list_rows
            .first()
            .expect("list row should exist")
            .split_whitespace()
            .nth(1)
            .expect("ratio column should be present")
            .parse::<f64>()
            .expect("ratio should parse");

        assert!(heavy_ratio > light_ratio);
    }

    #[test]
    fn genfmtjas_defaults_to_unit_weight_for_empty_q_grid() {
        let artifacts = genfmtjas(
            &GenfmtJasConfig {
                version_tag: "10.0".to_string(),
                critcw: 0.0,
                iorder: 2,
                q_weights: Vec::new(),
            },
            &[GenfmtPathInput {
                path_index: 1,
                nleg: 2,
                degeneracy: 1.0,
                reff: 1.0,
                amplitude: 2.0,
            }],
        );

        assert!(artifacts.list_rows[0].contains("100.000"));
    }
}
