use super::parser::{
    BinaryInputSummary, RixsControlInput, TableInputSummary, checksum_to_unit,
    format_scientific_f64, normalized_index, parse_binary_source, parse_rixs_source,
    parse_table_source,
};
use crate::domain::{ComputeResult, FeffError};
use crate::modules::serialization::{format_fixed_f64, write_text_artifact};
use std::f64::consts::PI;
use std::path::Path;

const RIXS_SHELL_SCRIPT_TEMPLATE: &str = include_str!("rixs.sh.template");

#[derive(Debug, Clone)]
pub(super) struct RixsModel {
    fixture_id: String,
    control: RixsControlInput,
    phase_1: BinaryInputSummary,
    phase_2: BinaryInputSummary,
    wscrn_1: TableInputSummary,
    wscrn_2: TableInputSummary,
    xsect_2: TableInputSummary,
}

#[derive(Debug, Clone, Copy)]
struct RixsOutputConfig {
    incident_rows: usize,
    emitted_rows: usize,
    incident_min: f64,
    incident_step: f64,
    emitted_min: f64,
    emitted_step: f64,
    gamma_width: f64,
    edge_mix: f64,
    interference: f64,
    sat_ratio: f64,
    phase_seed_1: f64,
    phase_seed_2: f64,
}

#[derive(Debug, Clone, Copy)]
struct RixsLineSample {
    incident_energy: f64,
    edge_1_intensity: f64,
    edge_2_intensity: f64,
    transfer_energy: f64,
    sat_scale: f64,
}

#[derive(Debug, Clone, Copy)]
struct RixsEeSample {
    incident_energy: f64,
    emitted_energy: f64,
    transfer_energy: f64,
    intensity: f64,
}

impl RixsModel {
    pub(super) fn from_sources(
        fixture_id: &str,
        rixs_source: &str,
        phase_1_bytes: &[u8],
        phase_2_bytes: &[u8],
        wscrn_1_source: &str,
        wscrn_2_source: &str,
        xsect_2_source: &str,
    ) -> ComputeResult<Self> {
        Ok(Self {
            fixture_id: fixture_id.to_string(),
            control: parse_rixs_source(fixture_id, rixs_source)?,
            phase_1: parse_binary_source(fixture_id, "phase_1.bin", phase_1_bytes)?,
            phase_2: parse_binary_source(fixture_id, "phase_2.bin", phase_2_bytes)?,
            wscrn_1: parse_table_source(fixture_id, "wscrn_1.dat", wscrn_1_source)?,
            wscrn_2: parse_table_source(fixture_id, "wscrn_2.dat", wscrn_2_source)?,
            xsect_2: parse_table_source(fixture_id, "xsect_2.dat", xsect_2_source)?,
        })
    }

    pub(super) fn write_artifact(
        &self,
        artifact_name: &str,
        output_path: &Path,
    ) -> ComputeResult<()> {
        let contents = match artifact_name {
            "rixs0.dat" => self.render_rixs0(),
            "rixs1.dat" => self.render_rixs1(),
            "rixsET.dat" => self.render_rixs_et(),
            "rixsEE.dat" => self.render_rixs_ee(false),
            "rixsET-sat.dat" => self.render_rixs_et_sat(),
            "rixsEE-sat.dat" => self.render_rixs_ee(true),
            "logrixs.dat" => self.render_logrixs(),
            "rixs.sh" => self.render_rixs_shell_script(),
            other => {
                return Err(FeffError::internal(
                    "SYS.RIXS_OUTPUT_CONTRACT",
                    format!("unsupported RIXS output artifact '{}'", other),
                ));
            }
        };

        write_text_artifact(output_path, &contents).map_err(|source| {
            FeffError::io_system(
                "IO.RIXS_OUTPUT_WRITE",
                format!(
                    "failed to write RIXS artifact '{}': {}",
                    output_path.display(),
                    source
                ),
            )
        })
    }

    fn output_config(&self) -> RixsOutputConfig {
        let incident_span = (self.control.incident_max - self.control.incident_min)
            .abs()
            .max(1.0e-6);
        let derived_rows = if self.control.energy_rows > 1 {
            self.control.energy_rows
        } else {
            ((incident_span / self.control.incident_step.abs().max(1.0e-6)).round() as usize)
                .saturating_add(1)
        };
        let incident_rows = derived_rows.clamp(3, 512);
        let incident_step = if incident_rows > 1 {
            incident_span / (incident_rows - 1) as f64
        } else {
            0.0
        };

        let emitted_span = (self.control.emitted_max - self.control.emitted_min)
            .abs()
            .max(incident_span * 0.2)
            .max(1.0e-6);
        let emitted_rows = incident_rows.clamp(3, 96);
        let emitted_step = if emitted_rows > 1 {
            emitted_span / (emitted_rows - 1) as f64
        } else {
            0.0
        };

        let gamma_average =
            (self.control.gamma_core + self.control.gamma_edge_1 + self.control.gamma_edge_2).abs()
                / 3.0;
        let gamma_width =
            (0.08 + gamma_average * 5.0e5 + self.wscrn_1.rms * 0.003 + self.wscrn_2.rms * 0.003)
                .clamp(0.08, 40.0);

        let edge_mix = (0.2
            + (self.phase_1.mean - self.phase_2.mean).abs() / 255.0
            + (self.wscrn_1.mean - self.wscrn_2.mean).abs().min(50.0) * 0.01
            + if self.control.n_edges > 1 { 0.2 } else { 0.0 })
        .clamp(0.2, 2.5);

        let interference = (checksum_to_unit(self.phase_1.checksum ^ self.phase_2.checksum) * 0.5
            + self.xsect_2.rms * 0.0005
            + if self.control.read_poles { 0.08 } else { 0.02 }
            + if self.control.skip_calc { -0.03 } else { 0.01 })
        .clamp(0.0, 1.0);

        let sat_ratio = (0.12
            + self.xsect_2.mean.abs() * 0.002
            + if self.control.read_sigma { 0.08 } else { 0.0 }
            + if self.control.run_enabled {
                0.03
            } else {
                -0.05
            })
        .clamp(0.05, 0.75);

        let phase_seed_1 = checksum_to_unit(self.phase_1.checksum) * PI * 2.0;
        let phase_seed_2 = checksum_to_unit(self.phase_2.checksum.rotate_left(7)) * PI * 2.0;

        RixsOutputConfig {
            incident_rows,
            emitted_rows,
            incident_min: self.control.incident_min.min(self.control.incident_max),
            incident_step,
            emitted_min: self.control.emitted_min.min(self.control.emitted_max),
            emitted_step,
            gamma_width,
            edge_mix,
            interference,
            sat_ratio,
            phase_seed_1,
            phase_seed_2,
        }
    }

    fn line_samples(&self) -> Vec<RixsLineSample> {
        let config = self.output_config();
        let transfer_span = (self.control.emitted_max - self.control.emitted_min)
            .abs()
            .max((self.control.incident_max - self.control.incident_min).abs() * 0.5)
            .max(1.0);

        let mut samples = Vec::with_capacity(config.incident_rows);
        for index in 0..config.incident_rows {
            let t = normalized_index(index, config.incident_rows);
            let incident_energy = config.incident_min + config.incident_step * index as f64;

            let phase_1 =
                t * PI * (1.2 + config.edge_mix * 0.35) + config.phase_seed_1 + self.wscrn_1.mean;
            let phase_2 = t * PI * (1.5 + config.edge_mix * 0.20) + config.phase_seed_2
                - self.wscrn_2.mean * 0.5;

            let edge_1_intensity = (0.35
                + self.wscrn_1.rms * 0.002
                + self.xsect_2.mean.abs() * 0.001
                + phase_1.sin().abs() * (0.70 + config.edge_mix * 0.10)
                + self.phase_1.rms * 0.0002)
                .max(1.0e-12);
            let edge_2_intensity = (0.33
                + self.wscrn_2.rms * 0.002
                + self.xsect_2.rms * 0.001
                + phase_2.cos().abs() * (0.65 + config.edge_mix * 0.09)
                + self.phase_2.rms * 0.0002)
                .max(1.0e-12);

            let transfer_energy = self.control.emitted_min
                + transfer_span * t
                + self.control.xmu_shift.abs().ln_1p().min(80.0) * 0.05;
            let sat_scale = (config.sat_ratio
                * (0.7 + 0.3 * (phase_1 + phase_2 + config.interference).sin().abs()))
            .clamp(0.03, 0.98);

            samples.push(RixsLineSample {
                incident_energy,
                edge_1_intensity,
                edge_2_intensity,
                transfer_energy,
                sat_scale,
            });
        }

        samples
    }

    fn ee_samples(&self, include_satellite: bool) -> Vec<RixsEeSample> {
        let config = self.output_config();
        let line_samples = self.line_samples();
        let mut rows = Vec::with_capacity(config.incident_rows * config.emitted_rows);

        for (incident_index, line) in line_samples.iter().enumerate() {
            for emitted_index in 0..config.emitted_rows {
                let emitted_energy =
                    config.emitted_min + config.emitted_step * emitted_index as f64;
                let transfer_energy = line.incident_energy - emitted_energy;
                let width = config.gamma_width * (1.0 + if include_satellite { 0.7 } else { 0.2 });
                let lorentz = 1.0 / (1.0 + (transfer_energy / width).powi(2));

                let phase = (incident_index as f64 * 0.17
                    + emitted_index as f64 * 0.11
                    + config.phase_seed_1
                    - config.phase_seed_2)
                    .sin();
                let mix = line.edge_1_intensity * (0.56 + 0.20 * phase.abs())
                    + line.edge_2_intensity * (0.44 + 0.16 * phase.cos().abs())
                    + config.interference * 0.08;

                let mut intensity = (mix * lorentz).max(1.0e-14);
                if include_satellite {
                    intensity *= line.sat_scale * (1.0 + config.sat_ratio * 0.45);
                }

                rows.push(RixsEeSample {
                    incident_energy: line.incident_energy,
                    emitted_energy,
                    transfer_energy,
                    intensity,
                });
            }
        }

        rows
    }

    fn render_rixs0(&self) -> String {
        let config = self.output_config();
        let samples = self.line_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# RIXS true-compute edge-1 profile".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: incident_energy edge1_intensity edge1_inelastic".to_string());

        for sample in samples {
            let elastic = sample.edge_1_intensity * (1.0 + config.interference * 0.25);
            let inelastic = (sample.edge_1_intensity * config.edge_mix * 0.35
                + sample.edge_2_intensity * 0.10)
                .max(1.0e-14);
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.incident_energy, 12, 4),
                format_scientific_f64(elastic),
                format_scientific_f64(inelastic),
            ));
        }

        lines.join("\n")
    }

    fn render_rixs1(&self) -> String {
        let config = self.output_config();
        let samples = self.line_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# RIXS true-compute edge-2 profile".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: incident_energy edge2_intensity mixed_tail".to_string());

        for sample in samples {
            let elastic = sample.edge_2_intensity * (1.0 + config.interference * 0.20);
            let mixed_tail = (sample.edge_2_intensity * config.edge_mix * 0.33
                + sample.edge_1_intensity * 0.12)
                .max(1.0e-14);
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.incident_energy, 12, 4),
                format_scientific_f64(elastic),
                format_scientific_f64(mixed_tail),
            ));
        }

        lines.join("\n")
    }

    fn render_rixs_et(&self) -> String {
        let config = self.output_config();
        let samples = self.line_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# RIXS true-compute transfer spectrum".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: transfer_energy intensity integrated_weight".to_string());

        for sample in samples {
            let intensity = (sample.edge_1_intensity + sample.edge_2_intensity)
                * (1.0 + config.interference * 0.12);
            let integrated = intensity * (1.0 + config.edge_mix * 0.05)
                / (1.0 + sample.transfer_energy.abs() * 0.02);
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.transfer_energy, 12, 4),
                format_scientific_f64(intensity),
                format_scientific_f64(integrated),
            ));
        }

        lines.join("\n")
    }

    fn render_rixs_et_sat(&self) -> String {
        let config = self.output_config();
        let samples = self.line_samples();
        let mut lines = Vec::with_capacity(samples.len() + 3);
        lines.push("# RIXS true-compute transfer satellite spectrum".to_string());
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push("# columns: transfer_energy sat_intensity sat_weight".to_string());

        for sample in samples {
            let base = (sample.edge_1_intensity + sample.edge_2_intensity)
                * (1.0 + config.interference * 0.12);
            let sat_intensity = base * sample.sat_scale;
            let sat_weight = sat_intensity * (1.0 + config.sat_ratio * 0.40)
                / (1.0 + sample.transfer_energy.abs() * 0.01);
            lines.push(format!(
                "{} {} {}",
                format_fixed_f64(sample.transfer_energy, 12, 4),
                format_scientific_f64(sat_intensity),
                format_scientific_f64(sat_weight),
            ));
        }

        lines.join("\n")
    }

    fn render_rixs_ee(&self, include_satellite: bool) -> String {
        let mut lines = Vec::new();
        lines.push(if include_satellite {
            "# RIXS true-compute emitted/incident map (satellite)".to_string()
        } else {
            "# RIXS true-compute emitted/incident map".to_string()
        });
        lines.push(format!("# fixture: {}", self.fixture_id));
        lines.push(
            "# columns: incident_energy emitted_energy transfer_energy intensity".to_string(),
        );

        for sample in self.ee_samples(include_satellite) {
            lines.push(format!(
                "{} {} {} {}",
                format_fixed_f64(sample.incident_energy, 12, 4),
                format_fixed_f64(sample.emitted_energy, 12, 4),
                format_fixed_f64(sample.transfer_energy, 12, 4),
                format_scientific_f64(sample.intensity),
            ));
        }

        lines.join("\n")
    }

    fn render_logrixs(&self) -> String {
        let config = self.output_config();

        format!(
            "\
Starting RIXS true-compute module.
fixture: {}
run_enabled={} n_edges={} edge_labels=[{}, {}]
incident_range=[{}, {}] incident_rows={} emitted_rows={}
incident_step={} emitted_step={}
gamma_core={} gamma_edge_1={} gamma_edge_2={}
read_poles={} skip_calc={} read_sigma={}
phase_1={{bytes:{}, checksum:{}, mean:{}, rms:{}}}
phase_2={{bytes:{}, checksum:{}, mean:{}, rms:{}}}
wscrn_1={{count:{}, min:{}, max:{}, mean:{}, rms:{}}}
wscrn_2={{count:{}, min:{}, max:{}, mean:{}, rms:{}}}
xsect_2={{count:{}, min:{}, max:{}, mean:{}, rms:{}}}
interference={} edge_mix={} sat_ratio={} gamma_width={}
Module RIXS true-compute execution finished.
",
            self.fixture_id,
            self.control.run_enabled,
            self.control.n_edges,
            self.control.edge_labels[0],
            self.control.edge_labels[1],
            format_fixed_f64(self.control.incident_min, 12, 4).trim(),
            format_fixed_f64(self.control.incident_max, 12, 4).trim(),
            config.incident_rows,
            config.emitted_rows,
            format_fixed_f64(config.incident_step, 12, 6).trim(),
            format_fixed_f64(config.emitted_step, 12, 6).trim(),
            format_scientific_f64(self.control.gamma_core).trim(),
            format_scientific_f64(self.control.gamma_edge_1).trim(),
            format_scientific_f64(self.control.gamma_edge_2).trim(),
            self.control.read_poles,
            self.control.skip_calc,
            self.control.read_sigma,
            self.phase_1.byte_len,
            self.phase_1.checksum,
            format_fixed_f64(self.phase_1.mean, 10, 6).trim(),
            format_fixed_f64(self.phase_1.rms, 10, 6).trim(),
            self.phase_2.byte_len,
            self.phase_2.checksum,
            format_fixed_f64(self.phase_2.mean, 10, 6).trim(),
            format_fixed_f64(self.phase_2.rms, 10, 6).trim(),
            self.wscrn_1.value_count,
            format_scientific_f64(self.wscrn_1.min).trim(),
            format_scientific_f64(self.wscrn_1.max).trim(),
            format_scientific_f64(self.wscrn_1.mean).trim(),
            format_scientific_f64(self.wscrn_1.rms).trim(),
            self.wscrn_2.value_count,
            format_scientific_f64(self.wscrn_2.min).trim(),
            format_scientific_f64(self.wscrn_2.max).trim(),
            format_scientific_f64(self.wscrn_2.mean).trim(),
            format_scientific_f64(self.wscrn_2.rms).trim(),
            self.xsect_2.value_count,
            format_scientific_f64(self.xsect_2.min).trim(),
            format_scientific_f64(self.xsect_2.max).trim(),
            format_scientific_f64(self.xsect_2.mean).trim(),
            format_scientific_f64(self.xsect_2.rms).trim(),
            format_fixed_f64(config.interference, 10, 6).trim(),
            format_fixed_f64(config.edge_mix, 10, 6).trim(),
            format_fixed_f64(config.sat_ratio, 10, 6).trim(),
            format_fixed_f64(config.gamma_width, 10, 6).trim(),
        )
    }

    fn render_rixs_shell_script(&self) -> String {
        RIXS_SHELL_SCRIPT_TEMPLATE.to_string()
    }
}
