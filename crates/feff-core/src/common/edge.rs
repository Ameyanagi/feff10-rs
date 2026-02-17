//! FEFF COMMON edge-label handling and core-hole lifetime helpers.
//!
//! This module ports the behavior from `feff10/src/COMMON/isedge.f90`,
//! `feff10/src/RDINP/setedg.f90`, and `feff10/src/COMMON/setgam.f90`.

const EDGE_LABELS: [&str; 41] = [
    "NO", "K", "L1", "L2", "L3", "M1", "M2", "M3", "M4", "M5", "N1", "N2", "N3", "N4", "N5", "N6",
    "N7", "O1", "O2", "O3", "O4", "O5", "O6", "O7", "O8", "O9", "P1", "P2", "P3", "P4", "P5", "P6",
    "P7", "R1", "R2", "R3", "R4", "R5", "S1", "S2", "S3",
];

const CORE_HOLE_Z_POINTS: [[f32; 8]; 16] = [
    [0.99, 10.0, 20.0, 40.0, 50.0, 60.0, 80.0, 95.1],
    [0.99, 18.0, 22.0, 35.0, 50.0, 52.0, 75.0, 95.1],
    [0.99, 17.0, 28.0, 31.0, 45.0, 60.0, 80.0, 95.1],
    [0.99, 17.0, 28.0, 31.0, 45.0, 60.0, 80.0, 95.1],
    [0.99, 20.0, 28.0, 30.0, 36.0, 53.0, 80.0, 95.1],
    [0.99, 20.0, 22.0, 30.0, 40.0, 68.0, 80.0, 95.1],
    [0.99, 20.0, 22.0, 30.0, 40.0, 68.0, 80.0, 95.1],
    [0.99, 36.0, 40.0, 48.0, 58.0, 76.0, 79.0, 95.1],
    [0.99, 36.0, 40.0, 48.0, 58.0, 76.0, 79.0, 95.1],
    [0.99, 30.0, 40.0, 47.0, 50.0, 63.0, 80.0, 95.1],
    [0.99, 40.0, 42.0, 49.0, 54.0, 70.0, 87.0, 95.1],
    [0.99, 40.0, 42.0, 49.0, 54.0, 70.0, 87.0, 95.1],
    [0.99, 40.0, 50.0, 55.0, 60.0, 70.0, 81.0, 95.1],
    [0.99, 40.0, 50.0, 55.0, 60.0, 70.0, 81.0, 95.1],
    [0.99, 71.0, 73.0, 79.0, 86.0, 90.0, 95.0, 100.0],
    [0.99, 71.0, 73.0, 79.0, 86.0, 90.0, 95.0, 100.0],
];

const CORE_HOLE_GAMMA_POINTS: [[f32; 8]; 16] = [
    [0.02, 0.28, 0.75, 4.8, 10.5, 21.0, 60.0, 105.0],
    [0.07, 3.9, 3.8, 7.0, 6.0, 3.7, 8.0, 19.0],
    [0.001, 0.12, 1.4, 0.8, 2.6, 4.1, 6.3, 10.5],
    [0.001, 0.12, 0.55, 0.7, 2.1, 3.5, 5.4, 9.0],
    [0.001, 1.0, 2.9, 2.2, 5.5, 10.0, 22.0, 22.0],
    [0.001, 0.001, 0.5, 2.0, 2.6, 11.0, 15.0, 16.0],
    [0.001, 0.001, 0.5, 2.0, 2.6, 11.0, 10.0, 10.0],
    [0.0006, 0.09, 0.07, 0.48, 1.0, 4.0, 2.7, 4.7],
    [0.0006, 0.09, 0.07, 0.48, 0.87, 2.2, 2.5, 4.3],
    [0.001, 0.001, 6.2, 7.0, 3.2, 12.0, 16.0, 13.0],
    [0.001, 0.001, 1.9, 16.0, 2.7, 13.0, 13.0, 8.0],
    [0.001, 0.001, 1.9, 16.0, 2.7, 13.0, 13.0, 8.0],
    [0.001, 0.001, 0.15, 0.1, 0.8, 8.0, 8.0, 5.0],
    [0.001, 0.001, 0.15, 0.1, 0.8, 8.0, 8.0, 5.0],
    [0.001, 0.001, 0.05, 0.22, 0.1, 0.16, 0.5, 0.9],
    [0.001, 0.001, 0.05, 0.22, 0.1, 0.16, 0.5, 0.9],
];

pub const MAX_HOLE_CODE: i32 = 40;

/// Equivalent to FEFF `isedge`: accepts known edge labels (`K`, `L3`, ...)
/// and hole-code numbers (`0..40`).
pub fn is_edge(spec: &str) -> bool {
    hole_code_from_edge_spec(spec).is_some()
}

/// Convert an edge label or numeric code string into a FEFF hole code.
pub fn hole_code_from_edge_spec(spec: &str) -> Option<i32> {
    let normalized = spec.trim();
    if normalized.is_empty() {
        return None;
    }

    if let Ok(code) = normalized.parse::<i32>() {
        if (0..=MAX_HOLE_CODE).contains(&code) {
            return Some(code);
        }
        return None;
    }

    let uppercase = normalized.to_ascii_uppercase();
    EDGE_LABELS
        .iter()
        .position(|label| *label == uppercase)
        .map(|index| index as i32)
}

pub fn edge_label_from_hole_code(hole_code: i32) -> Option<&'static str> {
    if !(0..=MAX_HOLE_CODE).contains(&hole_code) {
        return None;
    }
    Some(EDGE_LABELS[hole_code as usize])
}

/// FEFF `setgam` core-hole lifetime in eV.
///
/// - `hole_code <= 0` returns `0.0`.
/// - `hole_code > 16` returns `0.1` eV, matching FEFF's O-shell fallback.
pub fn core_hole_lifetime_ev(atomic_number: i32, hole_code: i32) -> f64 {
    if hole_code <= 0 {
        return 0.0;
    }
    if hole_code > 16 {
        return 0.1;
    }

    let hole_index = (hole_code - 1) as usize;
    let z_points_raw = &CORE_HOLE_Z_POINTS[hole_index];
    let gamma_points_raw = &CORE_HOLE_GAMMA_POINTS[hole_index];

    let mut z_points = [0.0; 8];
    let mut log_gamma = [0.0; 8];
    for index in 0..8 {
        z_points[index] = z_points_raw[index] as f64;
        log_gamma[index] = (gamma_points_raw[index] as f64).log10();
    }

    let interpolated_log = interpolate_linear_segment(
        atomic_number as f64,
        &z_points,
        &log_gamma,
        locate_segment(atomic_number as f64, &z_points),
    );
    10.0_f64.powf(interpolated_log)
}

fn locate_segment(value: f64, points: &[f64; 8]) -> usize {
    if value < points[0] {
        return 0;
    }

    for upper in 1..points.len() {
        if value < points[upper] {
            return upper - 1;
        }
    }

    points.len() - 2
}

fn interpolate_linear_segment(value: f64, x: &[f64; 8], y: &[f64; 8], segment: usize) -> f64 {
    let x0 = x[segment];
    let x1 = x[segment + 1];
    let y0 = y[segment];
    let y1 = y[segment + 1];
    y0 + (value - x0) * (y1 - y0) / (x1 - x0)
}

#[cfg(test)]
mod tests {
    use super::{
        core_hole_lifetime_ev, edge_label_from_hole_code, hole_code_from_edge_spec, is_edge,
    };

    #[test]
    fn edge_lookup_supports_labels_and_codes() {
        assert!(is_edge("K"));
        assert!(is_edge("l3"));
        assert!(is_edge("10"));
        assert!(is_edge("0"));
        assert!(!is_edge("41"));
        assert!(!is_edge("Q2"));

        assert_eq!(hole_code_from_edge_spec("K"), Some(1));
        assert_eq!(hole_code_from_edge_spec("L1"), Some(2));
        assert_eq!(hole_code_from_edge_spec("L2"), Some(3));
        assert_eq!(hole_code_from_edge_spec("L3"), Some(4));
        assert_eq!(hole_code_from_edge_spec(" 4 "), Some(4));
        assert_eq!(hole_code_from_edge_spec(""), None);

        assert_eq!(edge_label_from_hole_code(0), Some("NO"));
        assert_eq!(edge_label_from_hole_code(4), Some("L3"));
        assert_eq!(edge_label_from_hole_code(40), Some("S3"));
        assert_eq!(edge_label_from_hole_code(41), None);
    }

    #[test]
    fn core_hole_lifetime_matches_known_reference_values() {
        assert_close(1.729_188_184_905_79, core_hole_lifetime_ev(29, 1), 1.0e-12);
        assert_close(5.280_137_321_651_70, core_hole_lifetime_ev(29, 2), 1.0e-12);
        assert_close(1.161_757_139_293_51, core_hole_lifetime_ev(29, 3), 1.0e-12);
        assert_close(0.596_038_724_693_039, core_hole_lifetime_ev(29, 4), 1.0e-12);

        assert_eq!(core_hole_lifetime_ev(29, 0), 0.0);
        assert_eq!(core_hole_lifetime_ev(29, -1), 0.0);
        assert_eq!(core_hole_lifetime_ev(29, 17), 0.1);
    }

    fn assert_close(expected: f64, actual: f64, tolerance: f64) {
        let diff = (actual - expected).abs();
        assert!(
            diff <= tolerance,
            "expected={:.16e} actual={:.16e} diff={:.16e} tolerance={:.16e}",
            expected,
            actual,
            diff,
            tolerance
        );
    }
}
