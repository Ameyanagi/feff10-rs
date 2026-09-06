use std::path::Path;

use feff10::{FeffInput, FeffTable};

pub fn copper_input() -> FeffInput {
    let source = include_str!("../../../feff10-cli/examples/bundled/exafs-cu.inp")
        .replace("RPATH 5.5", "RPATH 5.2")
        .replace("PRINT 0 0 0 0 0 0", "PRINT 0 0 0 0 0 3");
    FeffInput::parse(&source).unwrap()
}

pub fn assert_copper_paths(dir: &Path) {
    let count = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| {
            name.len() == 12
                && name.starts_with("feff")
                && name.ends_with(".dat")
                && name.as_bytes()[4..8].iter().all(u8::is_ascii_digit)
        })
        .count();
    assert_eq!(
        count,
        14,
        "expected 14 Cu scattering-path files in {}",
        dir.display()
    );

    let first = std::fs::read_to_string(dir.join("feff0001.dat")).unwrap();
    let geometry: Vec<f64> = first
        .lines()
        .find(|line| line.contains("nleg, deg, reff"))
        .expect("missing first-path geometry")
        .split_whitespace()
        .take(3)
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(geometry[0], 2.0);
    assert_eq!(geometry[1], 12.0);
    assert!((geometry[2] - 2.5527).abs() < 0.0001);

    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if name.len() != 12 || !name.starts_with("feff") || !name.ends_with(".dat") {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap();
        let data = content
            .lines()
            .skip_while(|line| !line.contains("real[2*phc]"))
            .skip(1)
            .collect::<Vec<_>>()
            .join("\n");
        let table = FeffTable::parse_strict(&data).unwrap();
        assert!(table.nrows() > 50, "missing path data: {}", path.display());
        assert_eq!(table.ncols(), 7);
        assert!(
            table.columns.iter().flatten().all(|v| v.is_finite()),
            "nonfinite path data: {}",
            path.display()
        );
        assert!(
            table.columns[2].iter().any(|v| *v > 0.0),
            "zero path amplitude: {}",
            path.display()
        );
    }
    let chi = FeffTable::from_file_strict(dir.join("chi.dat")).unwrap();
    assert!(chi.nrows() > 50);
    assert!(chi.columns.iter().flatten().all(|v| v.is_finite()));
    assert!(
        chi.columns[1].iter().any(|v| v.abs() > 1e-8),
        "all-zero EXAFS spectrum"
    );
}
