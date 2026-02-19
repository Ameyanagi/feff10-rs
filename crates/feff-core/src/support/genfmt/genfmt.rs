use super::genfmtjas::{GenfmtJasConfig, genfmtjas};
use super::genfmtsub::{GenfmtPathInput, GenfmtSubConfig, genfmt as genfmt_subroutine};
use super::m_genfmt::GenfmtArtifacts;
use super::regenf::artifacts_consumable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenfmtMode {
    Exafs,
    Nrixs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenfmtRunConfig {
    pub mfeff: i32,
    pub mode: GenfmtMode,
    pub version_tag: String,
    pub critcw: f64,
    pub iorder: i32,
    pub wnstar: bool,
    pub q_weights: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenfmtRunOutput {
    pub logs: Vec<String>,
    pub artifacts: Option<GenfmtArtifacts>,
}

pub fn ffmod5(config: &GenfmtRunConfig, paths: &[GenfmtPathInput]) -> GenfmtRunOutput {
    let mut logs = Vec::new();

    if config.mfeff != 1 {
        logs.push(format!(
            "Skipping GENFMT because mfeff={} (only mfeff=1 runs path formatting)",
            config.mfeff
        ));
        return GenfmtRunOutput {
            logs,
            artifacts: None,
        };
    }

    logs.push("Calculating EXAFS parameters ...".to_string());
    let artifacts = match config.mode {
        GenfmtMode::Exafs => genfmt_subroutine(
            &GenfmtSubConfig {
                version_tag: config.version_tag.clone(),
                critcw: config.critcw,
                iorder: config.iorder,
                include_nstar: config.wnstar,
            },
            paths,
        ),
        GenfmtMode::Nrixs => genfmtjas(
            &GenfmtJasConfig {
                version_tag: config.version_tag.clone(),
                critcw: config.critcw,
                iorder: config.iorder,
                q_weights: config.q_weights.clone(),
            },
            paths,
        ),
    };
    if artifacts_consumable(&artifacts) {
        logs.push("Validated GENFMT artifacts for downstream consumption.".to_string());
    } else {
        logs.push("GENFMT artifacts failed downstream consumability checks.".to_string());
    }
    logs.push("Done with module: EXAFS parameters (GENFMT).".to_string());

    GenfmtRunOutput {
        logs,
        artifacts: Some(artifacts),
    }
}

#[cfg(test)]
mod tests {
    use super::{GenfmtMode, GenfmtRunConfig, ffmod5};
    use crate::support::genfmt::genfmtsub::GenfmtPathInput;

    fn sample_paths() -> Vec<GenfmtPathInput> {
        vec![
            GenfmtPathInput {
                path_index: 1,
                nleg: 2,
                degeneracy: 4.0,
                reff: 2.3,
                amplitude: 0.8,
            },
            GenfmtPathInput {
                path_index: 2,
                nleg: 3,
                degeneracy: 2.0,
                reff: 3.4,
                amplitude: 0.3,
            },
        ]
    }

    #[test]
    fn ffmod5_skips_when_mfeff_is_disabled() {
        let output = ffmod5(
            &GenfmtRunConfig {
                mfeff: 0,
                mode: GenfmtMode::Exafs,
                version_tag: "10.0".to_string(),
                critcw: 0.0,
                iorder: 2,
                wnstar: false,
                q_weights: Vec::new(),
            },
            &sample_paths(),
        );

        assert!(output.artifacts.is_none());
        assert!(output.logs[0].contains("Skipping GENFMT"));
    }

    #[test]
    fn ffmod5_dispatches_to_exafs_genfmt_subroutine() {
        let output = ffmod5(
            &GenfmtRunConfig {
                mfeff: 1,
                mode: GenfmtMode::Exafs,
                version_tag: "10.1".to_string(),
                critcw: 5.0,
                iorder: 2,
                wnstar: true,
                q_weights: Vec::new(),
            },
            &sample_paths(),
        );

        assert_eq!(output.logs.len(), 3);
        assert!(output.logs[1].contains("Validated GENFMT artifacts"));
        let artifacts = output.artifacts.expect("artifacts should be generated");
        assert_eq!(artifacts.list_rows.len(), 2);
        assert!(!artifacts.nstar_rows.is_empty());
    }

    #[test]
    fn ffmod5_dispatches_to_nrixs_genfmt_variant() {
        let output = ffmod5(
            &GenfmtRunConfig {
                mfeff: 1,
                mode: GenfmtMode::Nrixs,
                version_tag: "10.1".to_string(),
                critcw: 2.0,
                iorder: 2,
                wnstar: false,
                q_weights: vec![1.0, 0.5, 0.5],
            },
            &sample_paths(),
        );

        let artifacts = output.artifacts.expect("artifacts should be generated");
        assert!(artifacts.feff_header_lines[1].contains("nrixs"));
        assert!(artifacts.feff_header_lines[1].contains("q_count=3"));
    }
}
