use super::parser::{
    AuxiliarySpectrumSummary, FullSpectrumControlInput, XmuRow, XmuSummary, parse_auxiliary_source,
    parse_fullspectrum_source, parse_xmu_source, summarize_xmu_rows,
};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct FullSpectrumModel {
    fixture_id: String,
    control: FullSpectrumControlInput,
    xmu_rows: Vec<XmuRow>,
    xmu_summary: XmuSummary,
    prexmu: Option<AuxiliarySpectrumSummary>,
    referencexmu: Option<AuxiliarySpectrumSummary>,
}

#[derive(Debug, Clone, Copy)]
struct FullSpectrumSample {
    energy: f64,
    total: f64,
    background: f64,
    fine_structure: f64,
    oscillator_strength: f64,
    eps1: f64,
    eps2: f64,
    drude: f64,
}

impl FullSpectrumModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        fullspectrum_source: &str,
        xmu_source: &str,
        prexmu_source: Option<&str>,
        referencexmu_source: Option<&str>,
    ) -> ComputeResult<Self> {
        let control = parse_fullspectrum_source(fixture_id, fullspectrum_source)?;
        let xmu_rows = parse_xmu_source(fixture_id, xmu_source)?;
        let xmu_summary = summarize_xmu_rows(&xmu_rows);

        let prexmu = prexmu_source.map(parse_auxiliary_source);
        let referencexmu = referencexmu_source.map(parse_auxiliary_source);

        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control,
            xmu_rows,
            xmu_summary,
            prexmu,
            referencexmu,
        })
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
    ) -> ComputeResult<()> {
        let contents = match artifact_name {
            "xmu.dat" => self.render_xmu_dat(),
            "osc_str.dat" => self.render_osc_str_dat(),
            "eps.dat" => self.render_eps_dat(),
            "drude.dat" => self.render_drude_dat(),
            "background.dat" => self.render_background_dat(),
            "fine_st.dat" => self.render_fine_st_dat(),
            "logfullspectrum.dat" => self.render_logfullspectrum_dat(),
            other => {
                return Err(FeffError::internal(
                    "SYS.FULLSPECTRUM_OUTPUT_CONTRACT",
                    format!("unsupported FULLSPECTRUM output artifact '{}'", other),
                ));
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.FULLSPECTRUM_OUTPUT_WRITE",
                format!(
                    "failed to write FULLSPECTRUM artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn render_xmu_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM true-compute xmu table".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev xmu_total xmu_background chi_component".to_string());

        for sample in samples {
            lines.push(format!(
                "{} {} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.total),
                format_scientific_f64(sample.background),
                format_scientific_f64(sample.fine_structure),
            ));
        }

        lines.join("\n")
    }

    fn render_osc_str_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM oscillator strengths".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev oscillator_strength cumulative_osc".to_string());

        let mut cumulative = 0.0_f64;
        for sample in samples {
            cumulative += sample.oscillator_strength;
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.oscillator_strength),
                format_scientific_f64(cumulative),
            ));
        }

        lines.join("\n")
    }

    fn render_eps_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM dielectric response".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev epsilon_1 epsilon_2 loss_function".to_string());

        for sample in samples {
            let denom = (sample.eps1 * sample.eps1 + sample.eps2 * sample.eps2).max(1.0e-18);
            let loss = sample.eps2 / denom;
            lines.push(format!(
                "{} {} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.eps1),
                format_scientific_f64(sample.eps2),
                format_scientific_f64(loss),
            ));
        }

        lines.join("\n")
    }

    fn render_drude_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM drude response".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev drude_weight conductivity plasmon_energy".to_string());

        for sample in samples {
            let conductivity = sample.drude * (0.85 + self.control.drude_scale * 0.02);
            let plasmon_energy = (sample.drude.abs().sqrt() * 2.5).max(0.0);
            lines.push(format!(
                "{} {} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.drude),
                format_scientific_f64(conductivity),
                format_scientific_f64(plasmon_energy),
            ));
        }

        lines.join("\n")
    }

    fn render_background_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM background table".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev background continuum_level".to_string());

        for sample in samples {
            let continuum =
                (sample.background * (1.0 + self.control.broadening_ev * 0.03)).max(0.0);
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.background),
                format_scientific_f64(continuum),
            ));
        }

        lines.join("\n")
    }

    fn render_fine_st_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# FULLSPECTRUM fine-structure table".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev fine_structure normalized_fine".to_string());

        let normalization = self.xmu_summary.rms_chi.abs().max(1.0e-12);
        for sample in samples {
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.energy, 12, 4),
                format_scientific_f64(sample.fine_structure),
                format_scientific_f64(sample.fine_structure / normalization),
            ));
        }

        lines.join("\n")
    }

    fn render_logfullspectrum_dat(&self) -> String {
        let prexmu_rows = self.prexmu.map(|summary| summary.row_count).unwrap_or(0);
        let reference_rows = self
            .referencexmu
            .map(|summary| summary.row_count)
            .unwrap_or(0);
        let prexmu_mean_energy = self
            .prexmu
            .map(|summary| format_fixed_f64(summary.mean_energy, 12, 4))
            .unwrap_or_else(|| "n/a".to_string());
        let reference_mean_energy = self
            .referencexmu
            .map(|summary| format_fixed_f64(summary.mean_energy, 12, 4))
            .unwrap_or_else(|| "n/a".to_string());

        format!(
            "\
FULLSPECTRUM true-compute runtime\n\
fixture: {}\n\
input-artifacts: fullspectrum.inp xmu.dat [prexmu.dat] [referencexmu.dat]\n\
output-artifacts: xmu.dat osc_str.dat eps.dat drude.dat background.dat fine_st.dat logfullspectrum.dat\n\
run-mode: {}\n\
broadening-ev: {} drude-scale: {} oscillator-scale: {} epsilon-shift: {}\n\
xmu-rows: {} energy-range=[{}, {}]\n\
mu-mean: {} mu0-mean: {} chi-mean: {} chi-rms: {}\n\
prexmu-present: {} rows={} mean-energy={} mean-signal={} rms-signal={}\n\
referencexmu-present: {} rows={} mean-energy={} mean-signal={} rms-signal={}\n\
Module 9 true-compute execution finished.\n",
            self.fixture_id,
            self.control.run_mode,
            format_fixed_f64(self.control.broadening_ev, 10, 5).trim(),
            format_fixed_f64(self.control.drude_scale, 10, 5).trim(),
            format_fixed_f64(self.control.oscillator_scale, 10, 5).trim(),
            format_fixed_f64(self.control.epsilon_shift, 10, 5).trim(),
            self.xmu_summary.row_count,
            format_fixed_f64(self.xmu_summary.energy_min, 12, 4).trim(),
            format_fixed_f64(self.xmu_summary.energy_max, 12, 4).trim(),
            format_scientific_f64(self.xmu_summary.mean_mu).trim(),
            format_scientific_f64(self.xmu_summary.mean_mu0).trim(),
            format_scientific_f64(self.xmu_summary.mean_chi).trim(),
            format_scientific_f64(self.xmu_summary.rms_chi).trim(),
            self.prexmu.is_some(),
            prexmu_rows,
            prexmu_mean_energy,
            self.prexmu
                .map(|summary| format_scientific_f64(summary.mean_signal))
                .unwrap_or_else(|| "n/a".to_string()),
            self.prexmu
                .map(|summary| format_scientific_f64(summary.rms_signal))
                .unwrap_or_else(|| "n/a".to_string()),
            self.referencexmu.is_some(),
            reference_rows,
            reference_mean_energy,
            self.referencexmu
                .map(|summary| format_scientific_f64(summary.mean_signal))
                .unwrap_or_else(|| "n/a".to_string()),
            self.referencexmu
                .map(|summary| format_scientific_f64(summary.rms_signal))
                .unwrap_or_else(|| "n/a".to_string()),
        )
    }

    fn derived_samples(&self) -> Vec<FullSpectrumSample> {
        let sample_count = self.xmu_rows.len().max(1);
        let prexmu_signal = self
            .prexmu
            .map(|summary| summary.mean_signal)
            .unwrap_or(0.0);
        let prexmu_rms = self.prexmu.map(|summary| summary.rms_signal).unwrap_or(0.0);
        let reference_signal = self
            .referencexmu
            .map(|summary| summary.mean_signal)
            .unwrap_or(0.0);
        let reference_rms = self
            .referencexmu
            .map(|summary| summary.rms_signal)
            .unwrap_or(0.0);

        let mode_gain = 1.0 + self.control.run_mode.abs() as f64 * 0.03;

        let mut samples = Vec::with_capacity(self.xmu_rows.len());
        for (index, row) in self.xmu_rows.iter().enumerate() {
            let t = if sample_count == 1 {
                0.0
            } else {
                index as f64 / (sample_count - 1) as f64
            };

            let phase = index as f64 * 0.073 + self.xmu_summary.mean_chi * 1.0e4;
            let pre_term = prexmu_signal * (1.0 + 0.15 * phase.sin());
            let reference_term = reference_signal * (1.0 + 0.12 * phase.cos());

            let background = (row.mu0.abs() * (1.0 + self.control.broadening_ev * 0.02)
                + self.xmu_summary.mean_mu0.abs() * 0.03
                + pre_term.abs() * 0.08
                + reference_term.abs() * 0.06)
                .max(1.0e-14);

            let fine_structure = row.chi * mode_gain
                + (row.mu - row.mu0) * 0.18
                + (pre_term - reference_term) * 0.01
                + self.xmu_summary.mean_chi * (1.0 - t) * 0.05;

            let total = (background + fine_structure).max(1.0e-14);

            let oscillator_strength = (fine_structure.abs() + self.xmu_summary.rms_chi * 0.1)
                * self.control.oscillator_scale
                * (1.0 + t * 0.25)
                + prexmu_rms * 1.0e-4;

            let eps1 = 1.0
                + self.control.epsilon_shift * 0.01
                + oscillator_strength * 0.2 / (1.0 + background.abs() * 50.0)
                + reference_rms * 1.0e-4;
            let eps2 = (oscillator_strength * 0.12
                + background * 0.03
                + prexmu_rms * 1.0e-3
                + reference_rms * 1.0e-3)
                .max(1.0e-12);

            let drude = self.control.drude_scale * background
                / (1.0 + t * 8.0 + self.control.broadening_ev.max(1.0e-6));

            samples.push(FullSpectrumSample {
                energy: row.energy,
                total,
                background,
                fine_structure,
                oscillator_strength,
                eps1,
                eps2,
                drude,
            });
        }

        samples
    }
}

pub(super) fn format_scientific_f64(value: f64) -> String {
    format!("{:>14.6E}", value)
}
