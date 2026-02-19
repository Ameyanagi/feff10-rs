const EDGE_LABELS: [&str; 41] = [
    "NO", "K", "L1", "L2", "L3", "M1", "M2", "M3", "M4", "M5", "N1", "N2", "N3", "N4", "N5", "N6",
    "N7", "O1", "O2", "O3", "O4", "O5", "O6", "O7", "O8", "O9", "P1", "P2", "P3", "P4", "P5", "P6",
    "P7", "R1", "R2", "R3", "R4", "R5", "S1", "S2", "S3",
];

pub fn isedge(value: &str) -> bool {
    canonical_edge_label(value).is_some()
}

pub fn canonical_edge_label(value: &str) -> Option<&'static str> {
    let normalized = value.trim().to_ascii_uppercase();
    if normalized.is_empty() {
        return None;
    }

    if let Some(index) = parse_edge_index(&normalized) {
        return EDGE_LABELS.get(index).copied();
    }

    EDGE_LABELS
        .iter()
        .copied()
        .find(|label| normalized == *label)
}

fn parse_edge_index(value: &str) -> Option<usize> {
    let parsed = value.parse::<usize>().ok()?;
    (parsed < EDGE_LABELS.len()).then_some(parsed)
}

#[cfg(test)]
mod tests {
    use super::{canonical_edge_label, isedge};

    #[test]
    fn isedge_accepts_label_variants() {
        assert!(isedge("K"));
        assert!(isedge("l3"));
        assert!(isedge(" n7 "));
        assert!(!isedge("ZZ"));
    }

    #[test]
    fn isedge_accepts_numeric_edge_aliases() {
        assert!(isedge("0"));
        assert!(isedge("40"));
        assert!(!isedge("41"));
    }

    #[test]
    fn canonical_label_normalizes_numeric_aliases() {
        assert_eq!(canonical_edge_label("1"), Some("K"));
        assert_eq!(canonical_edge_label("35"), Some("R3"));
        assert_eq!(canonical_edge_label("0"), Some("NO"));
    }
}
