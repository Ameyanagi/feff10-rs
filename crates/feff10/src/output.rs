use std::path::Path;

use crate::error::Error;

/// Parsed xmu.dat output file.
#[derive(Debug, Clone)]
pub struct XmuDat {
    pub header: Vec<String>,
    pub columns: Vec<Vec<f64>>,
}

impl XmuDat {
    /// Parse xmu.dat content from a string.
    pub fn parse(content: &str) -> Result<Self, Error> {
        let mut header = Vec::new();
        let mut rows: Vec<Vec<f64>> = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('#') {
                header.push(line.to_string());
                continue;
            }
            let vals: Vec<f64> = line
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if !vals.is_empty() {
                rows.push(vals);
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
}
