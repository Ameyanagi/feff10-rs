use std::path::Path;

use crate::error::{Error, ParseError};

/// Parsed xmu.dat output file.
#[derive(Debug, Clone)]
pub struct XmuDat {
    pub header: Vec<String>,
    pub columns: Vec<Vec<f64>>,
}

impl XmuDat {
    /// Parse xmu.dat content from a string.
    pub fn parse(content: &str) -> Result<Self, Error> {
        Self::parse_impl(content, false)
    }

    /// Parse xmu.dat content from a string with strict validation.
    ///
    /// Strict mode rejects:
    /// - non-numeric tokens in data rows
    /// - inconsistent number of columns between rows
    pub fn parse_strict(content: &str) -> Result<Self, Error> {
        Self::parse_impl(content, true)
    }

    fn parse_impl(content: &str, strict: bool) -> Result<Self, Error> {
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

            if strict {
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
                                message: format!("invalid numeric token '{token}' in xmu.dat row"),
                            }));
                        }
                    }
                }

                if let Some(cols) = expected_cols {
                    if vals.len() != cols {
                        return Err(Error::Parse(ParseError {
                            line: line_num,
                            message: format!(
                                "inconsistent column count in xmu.dat row: expected {cols}, got {}",
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

        Ok(XmuDat { header, columns })
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

    /// Compare two spectra using the R-squared metric (replicates rsqr.py).
    ///
    /// `col_x` and `col_y` are 0-based column indices.
    /// Returns the average R-squared value.
    pub fn r_squared(&self, other: &XmuDat, col_x: usize, col_y: usize) -> f64 {
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

    #[test]
    fn parse_reference_xmu() {
        let content = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../feff10/examples/EXAFS/Cu/referencexmu.dat"
        ))
        .unwrap();
        let xmu = XmuDat::parse(&content).unwrap();

        assert!(!xmu.header.is_empty());
        assert!(xmu.columns.len() >= 4);
        assert!(xmu.columns[0].len() > 10);
    }

    #[test]
    fn r_squared_identical() {
        let content = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../feff10/examples/EXAFS/Cu/referencexmu.dat"
        ))
        .unwrap();
        let xmu = XmuDat::parse(&content).unwrap();

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
    fn interp_clamps_left() {
        let xp = vec![1.0, 2.0, 3.0];
        let fp = vec![10.0, 20.0, 30.0];
        assert!((interp(&xp, &fp, 0.0) - 10.0).abs() < 1e-10);
        assert!((interp(&xp, &fp, -5.0) - 10.0).abs() < 1e-10);
    }

    #[test]
    fn interp_clamps_right() {
        let xp = vec![1.0, 2.0, 3.0];
        let fp = vec![10.0, 20.0, 30.0];
        assert!((interp(&xp, &fp, 4.0) - 30.0).abs() < 1e-10);
        assert!((interp(&xp, &fp, 100.0) - 30.0).abs() < 1e-10);
    }

    #[test]
    fn interp_at_exact_points() {
        let xp = vec![0.0, 1.0, 2.0, 3.0];
        let fp = vec![5.0, 10.0, 15.0, 20.0];
        for (x, f) in xp.iter().zip(fp.iter()) {
            assert!(
                (interp(&xp, &fp, *x) - f).abs() < 1e-10,
                "interp at x={x} should be {f}"
            );
        }
    }

    #[test]
    fn parse_empty_content() {
        let xmu = XmuDat::parse("").unwrap();
        assert!(xmu.header.is_empty());
        assert!(xmu.columns.is_empty());
    }

    #[test]
    fn parse_header_only() {
        let xmu = XmuDat::parse("# header line 1\n# header line 2\n").unwrap();
        assert_eq!(xmu.header.len(), 2);
        assert!(xmu.columns.is_empty());
    }

    #[test]
    fn parse_simple_data() {
        let content = "# header\n1.0 2.0 3.0\n4.0 5.0 6.0\n";
        let xmu = XmuDat::parse(content).unwrap();
        assert_eq!(xmu.columns.len(), 3);
        assert_eq!(xmu.columns[0], vec![1.0, 4.0]);
        assert_eq!(xmu.columns[1], vec![2.0, 5.0]);
        assert_eq!(xmu.columns[2], vec![3.0, 6.0]);
    }

    #[test]
    fn r_squared_missing_columns() {
        let content = "1.0 2.0\n3.0 4.0\n";
        let xmu = XmuDat::parse(content).unwrap();
        // Column 5 doesn't exist
        let rsq = xmu.r_squared(&xmu, 0, 5);
        assert!(rsq.is_nan());
    }

    #[test]
    fn r_squared_no_overlap() {
        let c1 = "1.0 10.0\n2.0 20.0\n3.0 30.0\n";
        let c2 = "5.0 50.0\n6.0 60.0\n7.0 70.0\n";
        let xmu1 = XmuDat::parse(c1).unwrap();
        let xmu2 = XmuDat::parse(c2).unwrap();
        let rsq = xmu1.r_squared(&xmu2, 0, 1);
        assert!(rsq.is_nan(), "non-overlapping ranges should return NaN");
    }

    #[test]
    fn r_squared_different_data() {
        let c1 = "1.0 10.0\n2.0 20.0\n3.0 30.0\n4.0 40.0\n5.0 50.0\n";
        let c2 = "1.0 15.0\n2.0 25.0\n3.0 35.0\n4.0 45.0\n5.0 55.0\n";
        let xmu1 = XmuDat::parse(c1).unwrap();
        let xmu2 = XmuDat::parse(c2).unwrap();
        let rsq = xmu1.r_squared(&xmu2, 0, 1);
        assert!(rsq > 0.0, "different data should have non-zero R-squared");
        assert!(rsq.is_finite());
    }

    #[test]
    fn parse_skips_blank_lines() {
        let content = "# header\n\n1.0 2.0\n\n3.0 4.0\n\n";
        let xmu = XmuDat::parse(content).unwrap();
        assert_eq!(xmu.columns.len(), 2);
        assert_eq!(xmu.columns[0].len(), 2);
    }

    #[test]
    fn parse_strict_rejects_ragged_rows() {
        let content = "1.0 2.0 3.0\n4.0 5.0\n";
        let err = XmuDat::parse_strict(content).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("inconsistent column count"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn parse_strict_rejects_invalid_token() {
        let content = "1.0 2.0\n3.0 abc\n";
        let err = XmuDat::parse_strict(content).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("invalid numeric token"),
            "unexpected error: {msg}"
        );
    }
}
