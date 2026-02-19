use super::m_genfmt::GenfmtArtifacts;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenfMode {
    Exafs,
    Nrixs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegenfInput {
    pub mfeff: i32,
    pub mode: RegenfMode,
    pub elpty: f64,
    pub do_nrixs: bool,
    pub jinit: i32,
    pub jmax: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegenfState {
    pub run_genfmt: bool,
    pub nrixs_initialized: bool,
    pub jinit: i32,
    pub init_pdata: bool,
    pub init_str: bool,
    pub log_messages: Vec<String>,
}

pub fn regenf(input: &RegenfInput) -> RegenfState {
    let run_genfmt = input.mfeff == 1;
    let mut jinit = input.jinit;
    let mut log_messages = Vec::new();

    let should_init_nrixs = input.do_nrixs && input.mode == RegenfMode::Nrixs && run_genfmt;
    if should_init_nrixs {
        log_messages.push("Initialized NRIXS state for GENFMT execution".to_string());
        if input.elpty < 0.0 {
            jinit = input.jmax;
            log_messages
                .push("Spherical NRIXS averaging requested; promoting jinit to jmax".to_string());
        }
    }

    if run_genfmt {
        log_messages.push("Initialized pdata and str context".to_string());
    }

    RegenfState {
        run_genfmt,
        nrixs_initialized: should_init_nrixs,
        jinit,
        init_pdata: run_genfmt,
        init_str: run_genfmt,
        log_messages,
    }
}

pub fn artifacts_consumable(artifacts: &GenfmtArtifacts) -> bool {
    if artifacts.feff_header_lines.is_empty() || artifacts.list_header_lines.is_empty() {
        return false;
    }

    if artifacts
        .list_rows
        .iter()
        .any(|row| row.split_whitespace().count() < 5)
    {
        return false;
    }

    !artifacts
        .nstar_rows
        .iter()
        .any(|row| row.split_whitespace().count() < 2)
}

#[cfg(test)]
mod tests {
    use super::{RegenfInput, RegenfMode, artifacts_consumable, regenf};
    use crate::support::genfmt::m_genfmt::GenfmtArtifacts;

    #[test]
    fn regenf_promotes_jinit_for_spherical_nrixs() {
        let state = regenf(&RegenfInput {
            mfeff: 1,
            mode: RegenfMode::Nrixs,
            elpty: -1.0,
            do_nrixs: true,
            jinit: 1,
            jmax: 5,
        });

        assert!(state.run_genfmt);
        assert!(state.nrixs_initialized);
        assert_eq!(state.jinit, 5);
    }

    #[test]
    fn regenf_skips_initialization_when_mfeff_disabled() {
        let state = regenf(&RegenfInput {
            mfeff: 0,
            mode: RegenfMode::Exafs,
            elpty: 0.0,
            do_nrixs: false,
            jinit: 1,
            jmax: 3,
        });

        assert!(!state.run_genfmt);
        assert!(!state.init_pdata);
        assert!(!state.init_str);
    }

    #[test]
    fn consumable_artifacts_require_structured_rows() {
        let valid = GenfmtArtifacts {
            feff_header_lines: vec!["#_feff.bin".to_string()],
            list_header_lines: vec!["header".to_string()],
            list_rows: vec!["1 0.01 100.0 2.0 4 2.5".to_string()],
            nstar_rows: vec!["1 10.0".to_string()],
        };
        assert!(artifacts_consumable(&valid));

        let invalid = GenfmtArtifacts {
            feff_header_lines: vec!["#_feff.bin".to_string()],
            list_header_lines: vec!["header".to_string()],
            list_rows: vec!["bad".to_string()],
            nstar_rows: vec![],
        };
        assert!(!artifacts_consumable(&invalid));
    }
}
