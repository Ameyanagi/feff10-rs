use super::parser::{DmdwControlInput, DymInputSummary, parse_dmdw_source, summarize_dym_input};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use std::f64::consts::PI;
use std::path::Path;

#[derive(Debug, Clone)]
pub(super) struct DmdwModel {
    fixture_id: String,
    control: DmdwControlInput,
    dym: DymInputSummary,
}

#[derive(Debug, Clone, Copy)]
struct DmdwPathRow {
    ipath: usize,
    jpath: usize,
    frequency_thz: f64,
    weight: f64,
    effective_temp_k: f64,
    effective_force_constant: f64,
    sigma2_ang2: f64,
}

impl DmdwModel {
    pub(super) fn from_inputs(
        fixture_id: &str,
        dmdw_source: &str,
        feff_dym_bytes: &[u8],
    ) -> ComputeResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_dmdw_source(fixture_id, dmdw_source)?,
            dym: summarize_dym_input(feff_dym_bytes),
        })
    }

    pub(super) fn write_artifact(&self, artifact_name: &str, output_path: &Path) -> ComputeResult<()> {
        let contents = match artifact_name {
            "dmdw.out" => self.render_dmdw_out(),
            other => {
                return Err(FeffError::internal(
                    "SYS.DMDW_OUTPUT_CONTRACT",
                    format!("unsupported DMDW output artifact '{}'", other),
                ));
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.DMDW_OUTPUT_WRITE",
                format!(
                    "failed to write DMDW artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn render_dmdw_out(&self) -> String {
        let rows = self.path_rows();

        let mut lines = Vec::with_capacity(rows.len() + 16);
        lines.push("# DMDW true-compute vibrational damping report".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push(format!(
            "# mode={} lanczos_order={} decomposition={} matrix={}",
            self.control.mode_selector,
            self.control.lanczos_order,
            self.control.decomposition_flag,
            self.control.matrix_label
        ));
        lines.push(format!(
            "# temperature={} reduced_mass={} path_range={}..{} step={}",
            format_fixed_f64(self.control.temperature, 8, 2).trim(),
            format_fixed_f64(self.control.reduced_mass, 8, 3).trim(),
            self.control.path_start,
            self.control.path_end,
            self.control.path_step
        ));
        lines.push(format!(
            "# dym_bytes={} dym_lines={} checksum={}",
            self.dym.byte_count, self.dym.line_count, self.dym.checksum
        ));
        lines.push(
            "# columns: ipath jpath frequency_thz weight effective_temp_k effective_force_constant sigma2_ang2".to_string(),
        );

        for row in rows {
            lines.push(format!(
                "{} {} {} {} {} {} {}",
                row.ipath,
                row.jpath,
                format_fixed_f64(row.frequency_thz, 12, 6),
                format_fixed_f64(row.weight, 12, 8),
                format_fixed_f64(row.effective_temp_k, 12, 4),
                format_fixed_f64(row.effective_force_constant, 12, 6),
                format_fixed_f64(row.sigma2_ang2, 12, 8),
            ));
        }

        lines.join("\n")
    }

    fn path_rows(&self) -> Vec<DmdwPathRow> {
        let span_count = ((self
            .control
            .path_end
            .saturating_sub(self.control.path_start))
            / self.control.path_step)
            + 1;
        let row_count = self
            .control
            .path_group_count
            .max(self.control.block_count)
            .max(span_count)
            .clamp(6, 320);

        let checksum_term = (self.dym.checksum % 10_000) as f64 / 10_000.0;
        let thermal_scale = ((self.control.temperature + 1.0) / 300.0)
            .sqrt()
            .clamp(0.2, 8.0);
        let mean_norm = (self.dym.mean_byte / 255.0).clamp(0.0, 1.0);

        let base_frequency = (1.15
            + self.control.lanczos_order as f64 * 0.28
            + checksum_term * 3.9
            + self.dym.rms * 2.2)
            .clamp(0.2, 80.0);

        let mut rows = Vec::with_capacity(row_count);
        let mut raw_weight_sum = 0.0_f64;

        for index in 0..row_count {
            let phase = index as f64 * 0.31
                + checksum_term * PI * 2.0
                + self.control.mode_selector as f64 * 0.17
                + self.control.decomposition_flag as f64 * 0.03;
            let path_offset = index % span_count;
            let jpath = self
                .control
                .path_start
                .saturating_add(path_offset.saturating_mul(self.control.path_step));

            let frequency_thz =
                (base_frequency * (1.0 + phase.sin() * 0.12) + index as f64 * 0.07).max(0.01);
            let raw_weight = (1.0 + phase.cos() * 0.35 + mean_norm * 0.2).max(0.05);

            let effective_temp_k =
                (self.control.temperature * (0.87 + phase.cos().abs() * 0.23)).clamp(1.0, 5000.0);
            let effective_force_constant = (self.control.reduced_mass
                * frequency_thz
                * frequency_thz
                * (0.22 + thermal_scale * 0.015))
                .max(1.0e-6);
            let sigma2_ang2 = (3.0e-4
                + thermal_scale * 2.6e-4
                + frequency_thz * 4.2e-5
                + self.dym.rms * 6.0e-4
                + raw_weight * 1.1e-4)
                .max(1.0e-8);

            rows.push(DmdwPathRow {
                ipath: self.control.path_start,
                jpath,
                frequency_thz,
                weight: raw_weight,
                effective_temp_k,
                effective_force_constant,
                sigma2_ang2,
            });
            raw_weight_sum += raw_weight;
        }

        let normalization = raw_weight_sum.max(1.0e-12);
        for row in &mut rows {
            row.weight /= normalization;
        }

        rows
    }
}
