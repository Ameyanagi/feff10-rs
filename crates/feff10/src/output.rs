use std::io;
use std::path::{Path, PathBuf};

use crate::error::{Error, ParseError};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ParseMode {
    Permissive,
    Strict,
}

/// Parsed numeric FEFF table data (for files like xmu.dat, chi.dat, eels.dat, ldosNN.dat).
#[derive(Debug, Clone)]
pub struct FeffTable {
    pub header: Vec<String>,
    pub columns: Vec<Vec<f64>>,
}

impl FeffTable {
    /// Parse FEFF table content from a string (permissive mode).
    pub fn parse(content: &str) -> Result<Self, Error> {
        Self::parse_with_mode(content, ParseMode::Permissive)
    }

    /// Parse FEFF table content from a string with strict validation.
    ///
    /// Strict mode rejects:
    /// - non-numeric tokens in data rows
    /// - inconsistent number of columns between rows
    pub fn parse_strict(content: &str) -> Result<Self, Error> {
        Self::parse_with_mode(content, ParseMode::Strict)
    }

    fn parse_with_mode(content: &str, mode: ParseMode) -> Result<Self, Error> {
        let mut header = Vec::new();
        let mut rows: Vec<Vec<f64>> = Vec::new();
        let mut expected_cols: Option<usize> = None;

        for (line_idx, line) in content.lines().enumerate() {
            let line_num = line_idx + 1;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('#') {
                header.push(line.to_string());
                continue;
            }

            if mode == ParseMode::Strict {
                let tokens: Vec<&str> = line.split_whitespace().collect();
                if tokens.is_empty() {
                    continue;
                }

                let mut vals = Vec::with_capacity(tokens.len());
                for token in tokens {
                    match token.parse::<f64>() {
                        Ok(v) => vals.push(v),
                        Err(_) => {
                            return Err(Error::Parse(ParseError {
                                line: line_num,
                                message: format!("invalid numeric token '{token}' in data row"),
                            }));
                        }
                    }
                }

                if let Some(cols) = expected_cols {
                    if vals.len() != cols {
                        return Err(Error::Parse(ParseError {
                            line: line_num,
                            message: format!(
                                "inconsistent column count in data row: expected {cols}, got {}",
                                vals.len()
                            ),
                        }));
                    }
                } else {
                    expected_cols = Some(vals.len());
                }

                rows.push(vals);
            } else {
                let vals: Vec<f64> = line
                    .split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if !vals.is_empty() {
                    rows.push(vals);
                }
            }
        }

        // Transpose rows into columns
        let ncols = rows.first().map(|r| r.len()).unwrap_or(0);
        let mut columns = vec![Vec::with_capacity(rows.len()); ncols];
        for row in &rows {
            for (i, &val) in row.iter().enumerate() {
                if i < ncols {
                    columns[i].push(val);
                }
            }
        }

        Ok(FeffTable { header, columns })
    }

    /// Parse from a file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse from a file with strict validation.
    pub fn from_file_strict(path: impl AsRef<Path>) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)?;
        Self::parse_strict(&content)
    }

    /// Number of columns.
    pub fn ncols(&self) -> usize {
        self.columns.len()
    }

    /// Number of rows.
    pub fn nrows(&self) -> usize {
        self.columns.first().map(|c| c.len()).unwrap_or(0)
    }

    /// Get one column by zero-based index.
    pub fn column(&self, index: usize) -> Option<&[f64]> {
        self.columns.get(index).map(Vec::as_slice)
    }

    /// Compare two spectra using the R-squared metric (replicates rsqr.py).
    ///
    /// `col_x` and `col_y` are 0-based column indices.
    /// Returns the average R-squared value.
    pub fn r_squared(&self, other: &FeffTable, col_x: usize, col_y: usize) -> f64 {
        let (x1, y1) = match (self.columns.get(col_x), self.columns.get(col_y)) {
            (Some(x), Some(y)) => (x.as_slice(), y.as_slice()),
            _ => return f64::NAN,
        };
        let (x2, y2) = match (other.columns.get(col_x), other.columns.get(col_y)) {
            (Some(x), Some(y)) => (x.as_slice(), y.as_slice()),
            _ => return f64::NAN,
        };

        if x1.is_empty() || x2.is_empty() {
            return f64::NAN;
        }

        // Determine overlapping range
        let xmin = x1[0].max(x2[0]);
        let xmax = x1[x1.len() - 1].min(x2[x2.len() - 1]);
        if xmin >= xmax {
            return f64::NAN;
        }

        // Interpolate both onto 100 evenly spaced points
        let npts = 100;
        let mut rsqr_sum = 0.0;
        for i in 0..npts {
            let x = xmin + (xmax - xmin) * i as f64 / (npts - 1) as f64;
            let v1 = interp(x1, y1, x);
            let v2 = interp(x2, y2, x);
            let denom = v1 + v2;
            if denom.abs() > 1e-30 {
                rsqr_sum += (v2 - v1).powi(2) / denom.powi(2);
            }
        }

        rsqr_sum / npts as f64
    }
}

/// One atom-leg record in `paths.dat`.
#[derive(Debug, Clone, PartialEq)]
pub struct PathLeg {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub ipot: i32,
    pub label: String,
    pub rleg: f64,
    pub beta: f64,
    pub eta: f64,
}

/// One path block in `paths.dat`.
#[derive(Debug, Clone, PartialEq)]
pub struct PathEntry {
    pub index: u32,
    pub nleg: usize,
    pub degeneracy: f64,
    pub r: f64,
    pub legs: Vec<PathLeg>,
}

/// Parsed FEFF `paths.dat` output.
#[derive(Debug, Clone, PartialEq)]
pub struct PathsDat {
    pub header: Vec<String>,
    pub entries: Vec<PathEntry>,
}

impl PathsDat {
    /// Parse `paths.dat` content from a string.
    pub fn parse(content: &str) -> Result<Self, Error> {
        let lines: Vec<&str> = content.lines().collect();
        let mut header = Vec::new();
        let mut entries = Vec::new();
        let mut i = 0usize;

        while i < lines.len() {
            let line_num = i + 1;
            let line = lines[i].trim_end();
            let trimmed = line.trim();
            if trimmed.is_empty() {
                i += 1;
                continue;
            }

            if trimmed.contains("index, nleg, degeneracy, r=") {
                let (index, nleg, degeneracy, r) = parse_path_header_line(trimmed, line_num)?;
                i += 1;

                // Optional label row (x y z ipot rleg beta eta)
                while i < lines.len() {
                    let t = lines[i].trim();
                    if t.is_empty() {
                        i += 1;
                        continue;
                    }
                    if t.contains("ipot") && t.contains("rleg") {
                        i += 1;
                        continue;
                    }
                    break;
                }

                let mut legs = Vec::with_capacity(nleg);
                let mut parsed_legs = 0usize;
                while parsed_legs < nleg {
                    if i >= lines.len() {
                        return Err(Error::Parse(ParseError {
                            line: line_num,
                            message: format!(
                                "path {index} expected {nleg} leg rows, got {parsed_legs}"
                            ),
                        }));
                    }

                    let leg_trimmed = lines[i].trim();
                    if leg_trimmed.is_empty() {
                        i += 1;
                        continue;
                    }
                    if leg_trimmed.contains("index, nleg, degeneracy, r=") {
                        return Err(Error::Parse(ParseError {
                            line: i + 1,
                            message: format!(
                                "path {index} expected {nleg} leg rows, got {parsed_legs}"
                            ),
                        }));
                    }

                    let leg = parse_path_leg_line(lines[i], i + 1)?;
                    legs.push(leg);
                    parsed_legs += 1;
                    i += 1;
                }

                entries.push(PathEntry {
                    index,
                    nleg,
                    degeneracy,
                    r,
                    legs,
                });
                continue;
            }

            header.push(trimmed.to_string());
            i += 1;
        }

        Ok(PathsDat { header, entries })
    }

    /// Parse `paths.dat` from a file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, Error> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn total_degeneracy(&self) -> f64 {
        self.entries.iter().map(|e| e.degeneracy).sum()
    }

    pub fn max_r(&self) -> Option<f64> {
        self.entries
            .iter()
            .map(|e| e.r)
            .max_by(|a, b| a.total_cmp(b))
    }
}

fn parse_path_header_line(line: &str, line_num: usize) -> Result<(u32, usize, f64, f64), Error> {
    let (left, right) = line
        .split_once("index, nleg, degeneracy, r=")
        .ok_or_else(|| {
            Error::Parse(ParseError {
                line: line_num,
                message: "invalid path header line".to_string(),
            })
        })?;

    let fields: Vec<&str> = left.split_whitespace().collect();
    if fields.len() < 3 {
        return Err(Error::Parse(ParseError {
            line: line_num,
            message: "path header missing index/nleg/degeneracy".to_string(),
        }));
    }

    let index = fields[0].parse::<u32>().map_err(|_| {
        Error::Parse(ParseError {
            line: line_num,
            message: format!("invalid path index '{}'", fields[0]),
        })
    })?;
    let nleg = fields[1].parse::<usize>().map_err(|_| {
        Error::Parse(ParseError {
            line: line_num,
            message: format!("invalid nleg '{}'", fields[1]),
        })
    })?;
    let degeneracy = fields[2].parse::<f64>().map_err(|_| {
        Error::Parse(ParseError {
            line: line_num,
            message: format!("invalid degeneracy '{}'", fields[2]),
        })
    })?;

    let r_token = right.split_whitespace().next().ok_or_else(|| {
        Error::Parse(ParseError {
            line: line_num,
            message: "path header missing r value".to_string(),
        })
    })?;
    let r = r_token.parse::<f64>().map_err(|_| {
        Error::Parse(ParseError {
            line: line_num,
            message: format!("invalid path r value '{r_token}'"),
        })
    })?;

    Ok((index, nleg, degeneracy, r))
}

fn parse_path_leg_line(line: &str, line_num: usize) -> Result<PathLeg, Error> {
    // Common FEFF format has quoted atom label: ... ipot 'Cu    ' rleg beta eta
    if let Some((left, after_first_quote)) = line.split_once('\'')
        && let Some((label_raw, right)) = after_first_quote.split_once('\'')
    {
        let left_tokens: Vec<&str> = left.split_whitespace().collect();
        if left_tokens.len() < 4 {
            return Err(Error::Parse(ParseError {
                line: line_num,
                message: "path leg row missing x y z ipot fields".to_string(),
            }));
        }
        let right_tokens: Vec<&str> = right.split_whitespace().collect();
        if right_tokens.len() < 3 {
            return Err(Error::Parse(ParseError {
                line: line_num,
                message: "path leg row missing rleg beta eta fields".to_string(),
            }));
        }

        return Ok(PathLeg {
            x: parse_f64(left_tokens[0], line_num, "x")?,
            y: parse_f64(left_tokens[1], line_num, "y")?,
            z: parse_f64(left_tokens[2], line_num, "z")?,
            ipot: parse_i32(left_tokens[3], line_num, "ipot")?,
            label: label_raw.trim().to_string(),
            rleg: parse_f64(right_tokens[0], line_num, "rleg")?,
            beta: parse_f64(right_tokens[1], line_num, "beta")?,
            eta: parse_f64(right_tokens[2], line_num, "eta")?,
        });
    }

    // Fallback: x y z ipot label rleg beta eta
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 8 {
        return Err(Error::Parse(ParseError {
            line: line_num,
            message: "invalid path leg row".to_string(),
        }));
    }

    Ok(PathLeg {
        x: parse_f64(tokens[0], line_num, "x")?,
        y: parse_f64(tokens[1], line_num, "y")?,
        z: parse_f64(tokens[2], line_num, "z")?,
        ipot: parse_i32(tokens[3], line_num, "ipot")?,
        label: tokens[4].to_string(),
        rleg: parse_f64(tokens[5], line_num, "rleg")?,
        beta: parse_f64(tokens[6], line_num, "beta")?,
        eta: parse_f64(tokens[7], line_num, "eta")?,
    })
}

fn parse_f64(token: &str, line_num: usize, field: &str) -> Result<f64, Error> {
    token.parse::<f64>().map_err(|_| {
        Error::Parse(ParseError {
            line: line_num,
            message: format!("invalid {field} '{token}'"),
        })
    })
}

fn parse_i32(token: &str, line_num: usize, field: &str) -> Result<i32, Error> {
    token.parse::<i32>().map_err(|_| {
        Error::Parse(ParseError {
            line: line_num,
            message: format!("invalid {field} '{token}'"),
        })
    })
}

/// Classified output file kinds from a FEFF run directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputKind {
    Xmu,
    XmuSeries,
    Chi,
    ChiSeries,
    Eels,
    Ldos,
    Paths,
    GenericDat,
}

impl OutputKind {
    pub fn as_str(self) -> &'static str {
        match self {
            OutputKind::Xmu => "xmu",
            OutputKind::XmuSeries => "xmu_series",
            OutputKind::Chi => "chi",
            OutputKind::ChiSeries => "chi_series",
            OutputKind::Eels => "eels",
            OutputKind::Ldos => "ldos",
            OutputKind::Paths => "paths",
            OutputKind::GenericDat => "generic_dat",
        }
    }
}

/// One discovered output file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFile {
    pub name: String,
    pub path: PathBuf,
    pub kind: OutputKind,
}

/// Discovered FEFF outputs in a work directory.
#[derive(Debug, Clone)]
pub struct FeffOutputs {
    pub work_dir: PathBuf,
    pub files: Vec<OutputFile>,
}

impl FeffOutputs {
    /// Discover `*.dat` outputs in `work_dir`.
    pub fn discover(work_dir: impl AsRef<Path>) -> Result<Self, Error> {
        let work_dir = work_dir.as_ref().to_path_buf();
        let mut files = Vec::new();

        for entry in std::fs::read_dir(&work_dir)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if !ft.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(kind) = classify_output_file_name(&name) {
                files.push(OutputFile {
                    name,
                    path: entry.path(),
                    kind,
                });
            }
        }

        files.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(FeffOutputs { work_dir, files })
    }

    pub fn files(&self) -> &[OutputFile] {
        &self.files
    }

    pub fn file(&self, name: &str) -> Option<&OutputFile> {
        self.files.iter().find(|f| f.name == name)
    }

    pub fn of_kind(&self, kind: OutputKind) -> Vec<&OutputFile> {
        self.files.iter().filter(|f| f.kind == kind).collect()
    }

    pub fn read_table(&self, name: &str) -> Result<FeffTable, Error> {
        let path = self.resolve_file_path(name)?;
        FeffTable::from_file(path)
    }

    pub fn read_table_strict(&self, name: &str) -> Result<FeffTable, Error> {
        let path = self.resolve_file_path(name)?;
        FeffTable::from_file_strict(path)
    }

    pub fn read_paths(&self) -> Result<PathsDat, Error> {
        let path = self.resolve_file_path("paths.dat")?;
        PathsDat::from_file(path)
    }

    pub fn read_xmu(&self) -> Result<FeffTable, Error> {
        self.read_table("xmu.dat")
    }

    pub fn read_xmu_strict(&self) -> Result<FeffTable, Error> {
        self.read_table_strict("xmu.dat")
    }

    pub fn read_chi(&self) -> Result<FeffTable, Error> {
        self.read_table("chi.dat")
    }

    pub fn read_chi_strict(&self) -> Result<FeffTable, Error> {
        self.read_table_strict("chi.dat")
    }

    pub fn read_eels(&self) -> Result<FeffTable, Error> {
        self.read_table("eels.dat")
    }

    pub fn read_eels_strict(&self) -> Result<FeffTable, Error> {
        self.read_table_strict("eels.dat")
    }

    pub fn read_ldos(&self, index: u32) -> Result<FeffTable, Error> {
        self.read_table(&format!("ldos{index:02}.dat"))
    }

    pub fn read_ldos_strict(&self, index: u32) -> Result<FeffTable, Error> {
        self.read_table_strict(&format!("ldos{index:02}.dat"))
    }

    fn resolve_file_path(&self, name: &str) -> Result<&Path, Error> {
        self.file(name).map(|f| f.path.as_path()).ok_or_else(|| {
            Error::Io(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "output file '{}' not found in {}",
                    name,
                    self.work_dir.display()
                ),
            ))
        })
    }
}

/// Classify a FEFF output file name.
pub fn classify_output_file_name(name: &str) -> Option<OutputKind> {
    if !name.ends_with(".dat") {
        return None;
    }

    match name {
        "xmu.dat" => Some(OutputKind::Xmu),
        "chi.dat" => Some(OutputKind::Chi),
        "eels.dat" => Some(OutputKind::Eels),
        "paths.dat" => Some(OutputKind::Paths),
        _ => {
            if is_numbered_dat(name, "xmu") {
                Some(OutputKind::XmuSeries)
            } else if is_numbered_dat(name, "chi") {
                Some(OutputKind::ChiSeries)
            } else if is_numbered_dat(name, "ldos") {
                Some(OutputKind::Ldos)
            } else {
                Some(OutputKind::GenericDat)
            }
        }
    }
}

fn is_numbered_dat(name: &str, prefix: &str) -> bool {
    if let Some(rest) = name.strip_prefix(prefix)
        && let Some(digits) = rest.strip_suffix(".dat")
    {
        return !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit());
    }
    false
}

/// Linear interpolation (same as numpy.interp).
fn interp(xp: &[f64], fp: &[f64], x: f64) -> f64 {
    if x <= xp[0] {
        return fp[0];
    }
    if x >= xp[xp.len() - 1] {
        return fp[fp.len() - 1];
    }
    // Binary search for the interval
    let mut lo = 0;
    let mut hi = xp.len() - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if xp[mid] <= x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let t = (x - xp[lo]) / (xp[hi] - xp[lo]);
    fp[lo] + t * (fp[hi] - fp[lo])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_submodule() -> bool {
        std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../feff10/examples/EXAFS/Cu/referencexmu.dat"
        ))
        .exists()
    }

    #[test]
    fn parse_reference_xmu() {
        if !has_submodule() {
            return;
        }
        let content = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../feff10/examples/EXAFS/Cu/referencexmu.dat"
        ))
        .unwrap();
        let xmu = FeffTable::parse(&content).unwrap();

        assert!(!xmu.header.is_empty());
        assert!(xmu.columns.len() >= 4);
        assert!(xmu.columns[0].len() > 10);
    }

    #[test]
    fn r_squared_identical() {
        if !has_submodule() {
            return;
        }
        let content = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../feff10/examples/EXAFS/Cu/referencexmu.dat"
        ))
        .unwrap();
        let xmu = FeffTable::parse(&content).unwrap();

        // Identical spectra should have R-squared = 0
        let rsq = xmu.r_squared(&xmu, 0, 3);
        assert!(rsq < 1e-10, "Expected ~0 for identical spectra, got {rsq}");
    }

    #[test]
    fn interp_basic() {
        let xp = vec![0.0, 1.0, 2.0];
        let fp = vec![0.0, 10.0, 20.0];
        assert!((interp(&xp, &fp, 0.5) - 5.0).abs() < 1e-10);
        assert!((interp(&xp, &fp, 1.5) - 15.0).abs() < 1e-10);
    }

    #[test]
    fn parse_empty_content() {
        let xmu = FeffTable::parse("").unwrap();
        assert!(xmu.header.is_empty());
        assert!(xmu.columns.is_empty());
    }

    #[test]
    fn parse_header_only() {
        let xmu = FeffTable::parse("# header line 1\n# header line 2\n").unwrap();
        assert_eq!(xmu.header.len(), 2);
        assert!(xmu.columns.is_empty());
    }

    #[test]
    fn parse_simple_data() {
        let content = "# header\n1.0 2.0 3.0\n4.0 5.0 6.0\n";
        let xmu = FeffTable::parse(content).unwrap();
        assert_eq!(xmu.columns.len(), 3);
        assert_eq!(xmu.columns[0], vec![1.0, 4.0]);
        assert_eq!(xmu.columns[1], vec![2.0, 5.0]);
        assert_eq!(xmu.columns[2], vec![3.0, 6.0]);
    }

    #[test]
    fn parse_strict_rejects_ragged_rows() {
        let content = "1.0 2.0 3.0\n4.0 5.0\n";
        let err = FeffTable::parse_strict(content).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("inconsistent column count"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn parse_strict_rejects_invalid_token() {
        let content = "1.0 2.0\n3.0 abc\n";
        let err = FeffTable::parse_strict(content).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("invalid numeric token"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn classify_output_names() {
        assert_eq!(classify_output_file_name("xmu.dat"), Some(OutputKind::Xmu));
        assert_eq!(
            classify_output_file_name("xmu03.dat"),
            Some(OutputKind::XmuSeries)
        );
        assert_eq!(classify_output_file_name("chi.dat"), Some(OutputKind::Chi));
        assert_eq!(
            classify_output_file_name("chi09.dat"),
            Some(OutputKind::ChiSeries)
        );
        assert_eq!(
            classify_output_file_name("ldos00.dat"),
            Some(OutputKind::Ldos)
        );
        assert_eq!(
            classify_output_file_name("paths.dat"),
            Some(OutputKind::Paths)
        );
        assert_eq!(
            classify_output_file_name("vtot.dat"),
            Some(OutputKind::GenericDat)
        );
        assert_eq!(classify_output_file_name("feff.inp"), None);
    }

    #[test]
    fn paths_parse_basic() {
        let content = r#"
PATH  Rmax= 5.500,  Keep_limit= 0.00, Heap_limit 0.00  Pwcrit= 2.50%
 -----------------------------------------------------------------------
     1    2  12.000  index, nleg, degeneracy, r=  2.5527
      x           y           z     ipot  label      rleg      beta        eta
   -1.805000   -1.805000    0.000000   1 'Cu    '     2.5527  180.0000    0.0000
    0.000000    0.000000    0.000000   0 'Cu    '     2.5527  180.0000    0.0000
     2    2   6.000  index, nleg, degeneracy, r=  3.6100
      x           y           z     ipot  label      rleg      beta        eta
   -3.610000    0.000000    0.000000   1 'Cu    '     3.6100  180.0000    0.0000
    0.000000    0.000000    0.000000   0 'Cu    '     3.6100  180.0000    0.0000
"#;
        let paths = PathsDat::parse(content).unwrap();
        assert_eq!(paths.entries.len(), 2);
        assert_eq!(paths.entries[0].index, 1);
        assert_eq!(paths.entries[0].nleg, 2);
        assert_eq!(paths.entries[0].legs.len(), 2);
        assert_eq!(paths.entries[0].legs[0].label, "Cu");
        assert!((paths.entries[1].r - 3.6100).abs() < 1e-6);
        assert!((paths.total_degeneracy() - 18.0).abs() < 1e-6);
    }

    #[test]
    fn discover_outputs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("xmu.dat"), "#h\n1 2\n").unwrap();
        std::fs::write(dir.path().join("chi03.dat"), "1 2\n").unwrap();
        std::fs::write(dir.path().join("paths.dat"), "").unwrap();
        std::fs::write(dir.path().join("notes.txt"), "ignore").unwrap();

        let outputs = FeffOutputs::discover(dir.path()).unwrap();
        assert_eq!(outputs.files.len(), 3);
        assert!(outputs.file("xmu.dat").is_some());
        assert_eq!(outputs.of_kind(OutputKind::Paths).len(), 1);
    }
}
