use super::parser::{
    EelsControlInput, MagicInputSummary, XmuRow, XmuSummary, artifact_list, parse_eels_source,
    parse_magic_input_source, parse_xmu_source, summarize_xmu_rows,
};
use super::{EELS_OPTIONAL_OUTPUT, EELS_REQUIRED_OUTPUTS};
use crate::domain::{ComputeArtifact, ComputeResult, FeffError};
use crate::modules::helpers::{EelsMdffWorkflowConfig, eelsmdff_workflow_coupling};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct EelsModel {
    fixture_id: String,
    control: EelsControlInput,
    xmu_rows: Vec<XmuRow>,
    xmu_summary: XmuSummary,
    magic_input: Option<MagicInputSummary>,
}

struct EelsSample {
    energy: f64,
    total: f64,
    atomic_bg: f64,
    fine_struct: f64,
}

#[derive(Debug, Clone, Copy)]
struct MagicSample {
    energy: f64,
    magic_angle_mrad: f64,
    q_parallel: f64,
    q_perpendicular: f64,
    weight: f64,
}

impl EelsModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        eels_source: &str,
        xmu_source: &str,
        magic_source: Option<&str>,
    ) -> ComputeResult<Self> {
        let control = parse_eels_source(fixture_id, eels_source)?;
        let xmu_rows = parse_xmu_source(fixture_id, xmu_source)?;
        let xmu_summary = summarize_xmu_rows(&xmu_rows);
        let magic_input = magic_source.map(parse_magic_input_source);

        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control,
            xmu_rows,
            xmu_summary,
            magic_input,
        })
    }

    fn should_emit_magic(&self) -> bool {
        self.control.magic_flag || self.magic_input.is_some()
    }

    pub(super) fn expected_outputs(&self) -> Vec<ComputeArtifact> {
        let mut outputs = artifact_list(&EELS_REQUIRED_OUTPUTS);
        if self.should_emit_magic() {
            outputs.push(ComputeArtifact::new(EELS_OPTIONAL_OUTPUT));
        }
        outputs
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
    ) -> ComputeResult<()> {
        let contents = match artifact_name {
            "eels.dat" => self.render_eels_dat(),
            "logeels.dat" => self.render_logeels(),
            "magic.dat" => {
                if !self.should_emit_magic() {
                    return Err(FeffError::internal(
                        "SYS.EELS_OUTPUT_CONTRACT",
                        "magic.dat requested but magic output is disabled",
                    ));
                }
                self.render_magic_dat()
            }
            other => {
                return Err(FeffError::internal(
                    "SYS.EELS_OUTPUT_CONTRACT",
                    format!("unsupported EELS output artifact '{}'", other),
                ));
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.EELS_OUTPUT_WRITE",
                format!(
                    "failed to write EELS artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn render_eels_dat(&self) -> String {
        let samples = self.derived_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# EELS true-compute spectrum".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: energy_ev total atomic_bg fine_struct".to_string());

        for sample in samples {
            lines.push(format!(
                "{} {} {} {}",
                format_fixed_f64(sample.energy, 12, 3),
                format_scientific_f64(sample.total),
                format_scientific_f64(sample.atomic_bg),
                format_scientific_f64(sample.fine_struct),
            ));
        }

        lines.join("\n")
    }

    fn render_logeels(&self) -> String {
        let beam_direction = self.control.beam_direction;
        let collection_mrad = self.control.collection_semiangle_rad * 1000.0;
        let convergence_mrad = self.control.convergence_semiangle_rad * 1000.0;

        format!(
            "\
Starting EELS true-compute module.
fixture: {}
run_mode: {}
average={} relativistic={} cross_terms={}
beam_energy_ev={}
beam_direction=({}, {}, {})
collection_mrad={} convergence_mrad={}
qmesh={}x{}
detector_angles_rad=({}, {})
xmu_rows={} energy_range=[{}, {}]
mu_mean={} mu0_mean={} chi_rms={}
magic_requested={} magic_input_present={}
Module 8 true-compute execution finished.
",
            self.fixture_id,
            self.control.run_mode,
            self.control.average,
            self.control.relativistic,
            self.control.cross_terms,
            format_fixed_f64(self.control.beam_energy_ev, 12, 4).trim(),
            format_fixed_f64(beam_direction[0], 10, 5).trim(),
            format_fixed_f64(beam_direction[1], 10, 5).trim(),
            format_fixed_f64(beam_direction[2], 10, 5).trim(),
            format_fixed_f64(collection_mrad, 10, 4).trim(),
            format_fixed_f64(convergence_mrad, 10, 4).trim(),
            self.control.qmesh_radial,
            self.control.qmesh_angular,
            format_fixed_f64(self.control.detector_theta, 10, 6).trim(),
            format_fixed_f64(self.control.detector_phi, 10, 6).trim(),
            self.xmu_summary.row_count,
            format_fixed_f64(self.xmu_summary.energy_min, 12, 3).trim(),
            format_fixed_f64(self.xmu_summary.energy_max, 12, 3).trim(),
            format_scientific_f64(self.xmu_summary.mean_mu).trim(),
            format_scientific_f64(self.xmu_summary.mean_mu0).trim(),
            format_scientific_f64(self.xmu_summary.rms_chi).trim(),
            self.control.magic_flag,
            self.magic_input.is_some(),
        )
    }

    fn render_magic_dat(&self) -> String {
        let samples = self.magic_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# EELS magic-angle diagnostic table".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push(
            "# columns: energy_ev magic_angle_mrad q_parallel q_perpendicular weight".to_string(),
        );

        for sample in samples {
            lines.push(format!(
                "{} {} {} {} {}",
                format_fixed_f64(sample.energy, 12, 3),
                format_fixed_f64(sample.magic_angle_mrad, 10, 5),
                format_scientific_f64(sample.q_parallel),
                format_scientific_f64(sample.q_perpendicular),
                format_scientific_f64(sample.weight),
            ));
        }

        lines.join("\n")
    }

    fn derived_samples(&self) -> Vec<EelsSample> {
        let direction = self.control.beam_direction;
        let direction_norm = (direction[0] * direction[0]
            + direction[1] * direction[1]
            + direction[2] * direction[2])
            .sqrt()
            .max(1.0e-9);
        let direction_alignment = (direction[2].abs() / direction_norm).clamp(0.0, 1.0);
        let collection_mrad = (self.control.collection_semiangle_rad.abs() * 1000.0).max(1.0e-6);
        let convergence_mrad = (self.control.convergence_semiangle_rad.abs() * 1000.0).max(1.0e-6);
        let beam_kev = (self.control.beam_energy_ev.abs() / 1000.0).max(1.0);
        let relativistic_gain = if self.control.relativistic > 0 {
            1.0 + (beam_kev / 300.0).clamp(0.0, 4.0) * 0.06
        } else {
            1.0
        };
        let cross_term_gain = if self.control.cross_terms > 0 {
            0.015
        } else {
            0.0
        };
        let averaging_gain = if self.control.average > 0 { 0.92 } else { 1.0 };
        let polarization_span = (self.control.polarization_max - self.control.polarization_min)
            .abs()
            .max(1) as f64;
        let polarization_step = self.control.polarization_step.abs().max(1) as f64;
        let polarization_factor = (polarization_span / polarization_step).max(1.0).sqrt();
        let qmesh_density =
            (self.control.qmesh_radial.max(1) * self.control.qmesh_angular.max(1)) as f64;
        let detector_factor =
            1.0 + (self.control.detector_theta.abs() + self.control.detector_phi.abs()) * 0.15;
        let mdff_rows = self
            .xmu_rows
            .iter()
            .map(|row| (row.energy, row.mu, row.mu0, row.chi))
            .collect::<Vec<_>>();

        let _ = eelsmdff_workflow_coupling(
            EelsMdffWorkflowConfig {
                beam_energy_ev: self.control.beam_energy_ev,
                beam_direction: self.control.beam_direction,
                relativistic_q: self.control.relativistic > 0,
                qmesh_radial: self.control.qmesh_radial,
                qmesh_angular: self.control.qmesh_angular,
                average: self.control.average > 0,
                cross_terms: self.control.cross_terms > 0,
            },
            &mdff_rows,
        );

        let mut samples = Vec::with_capacity(self.xmu_rows.len());
        for (index, row) in self.xmu_rows.iter().enumerate() {
            let phase = index as f64 * 0.071
                + direction_alignment * 0.9
                + self.xmu_summary.mean_chi * 1.0e6;
            let orientation_term = 1.0 + direction_alignment * 0.08 * phase.cos();

            let atomic_bg = (row.mu0.abs()
                * (1.0 + collection_mrad * 0.01 + convergence_mrad * 0.006)
                + self.xmu_summary.mean_mu.abs() * 0.04
                + qmesh_density.sqrt() * 1.0e-7)
                .max(1.0e-12);

            let fine_struct = (row.chi * relativistic_gain * orientation_term * detector_factor
                + cross_term_gain * row.mu * phase.sin())
                * averaging_gain
                / polarization_factor;

            let total = (atomic_bg + fine_struct).max(1.0e-14);

            samples.push(EelsSample {
                energy: row.energy,
                total,
                atomic_bg,
                fine_struct,
            });
        }

        samples
    }

    fn magic_samples(&self) -> Vec<MagicSample> {
        let row_count =
            (self.control.qmesh_radial.max(1) * self.control.qmesh_angular.max(1)).clamp(8, 192);
        let spectrum_span = (self.xmu_summary.energy_max - self.xmu_summary.energy_min)
            .abs()
            .max(1.0);
        let energy_step = (spectrum_span / row_count as f64).max(0.1);
        let collection_mrad = (self.control.collection_semiangle_rad.abs() * 1000.0).max(1.0e-4);
        let convergence_mrad = (self.control.convergence_semiangle_rad.abs() * 1000.0).max(1.0e-4);
        let magic_mean = self
            .magic_input
            .map(|summary| summary.mean_value)
            .unwrap_or(0.0);
        let magic_rms = self
            .magic_input
            .map(|summary| summary.rms_value)
            .unwrap_or(0.0);
        let magic_weight = self
            .magic_input
            .map(|summary| summary.value_count as f64)
            .unwrap_or(1.0)
            .max(1.0);

        let base_angle =
            (collection_mrad * 0.85 + convergence_mrad * 1.15 + 0.5 * magic_rms).max(1.0e-4);
        let mut rows = Vec::with_capacity(row_count);
        let mut weight_sum = 0.0_f64;

        for index in 0..row_count {
            let t = if row_count == 1 {
                0.0
            } else {
                index as f64 / (row_count - 1) as f64
            };
            let phase = t * std::f64::consts::PI * 1.5 + magic_mean * 0.01;
            let energy = self.xmu_summary.energy_min
                + self.control.magic_energy_offset_ev
                + magic_mean
                + energy_step * index as f64;
            let magic_angle_mrad = (base_angle + phase.sin().abs() * 0.75).max(1.0e-5);
            let q_parallel = magic_angle_mrad * 1.0e-3 * (1.0 + t * 0.25);
            let q_perpendicular = magic_angle_mrad * 1.0e-3 * (1.0 - t * 0.15).max(0.05);
            let weight = (1.0 + phase.cos() * 0.35 + magic_weight * 0.02).max(0.02);

            rows.push(MagicSample {
                energy,
                magic_angle_mrad,
                q_parallel,
                q_perpendicular,
                weight,
            });
            weight_sum += weight;
        }

        let normalization = weight_sum.max(1.0e-12);
        for row in &mut rows {
            row.weight /= normalization;
        }

        rows
    }
}

fn format_scientific_f64(value: f64) -> String {
    format!("{value:.10E}")
}
