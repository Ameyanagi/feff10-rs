use feff10::output::XmuDat;
use pyo3::prelude::*;

use crate::error::to_pyerr;

#[pyclass(name = "XmuDat")]
#[derive(Clone)]
pub struct PyXmuDat {
    pub(crate) inner: XmuDat,
}

#[pymethods]
impl PyXmuDat {
    /// Parse xmu.dat content from a string.
    #[staticmethod]
    fn parse(content: &str) -> PyResult<Self> {
        XmuDat::parse(content)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Parse xmu.dat content from a string with strict validation.
    #[staticmethod]
    fn parse_strict(content: &str) -> PyResult<Self> {
        XmuDat::parse_strict(content)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Parse from a file path.
    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        XmuDat::from_file(path)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Parse from a file path with strict validation.
    #[staticmethod]
    fn from_file_strict(path: &str) -> PyResult<Self> {
        XmuDat::from_file_strict(path)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Header comment lines.
    #[getter]
    fn header(&self) -> Vec<String> {
        self.inner.header.clone()
    }

    /// All columns as a list of lists of floats.
    #[getter]
    fn columns(&self) -> Vec<Vec<f64>> {
        self.inner.columns.clone()
    }

    /// Number of columns.
    #[getter]
    fn ncols(&self) -> usize {
        self.inner.columns.len()
    }

    /// Number of data points (rows).
    #[getter]
    fn nrows(&self) -> usize {
        self.inner.columns.first().map(|c| c.len()).unwrap_or(0)
    }

    /// Get a specific column by index.
    fn column(&self, index: usize) -> PyResult<Vec<f64>> {
        self.inner
            .columns
            .get(index)
            .cloned()
            .ok_or_else(|| {
                pyo3::exceptions::PyIndexError::new_err(format!(
                    "column index {index} out of range (ncols={})",
                    self.inner.columns.len()
                ))
            })
    }

    /// Compare with another XmuDat using R-squared metric.
    ///
    /// Args:
    ///     other: Another XmuDat to compare against.
    ///     col_x: 0-based column index for the x-axis (energy).
    ///     col_y: 0-based column index for the y-axis (spectrum).
    ///
    /// Returns the average R-squared value. Returns NaN if columns are
    /// missing or spectra don't overlap.
    fn r_squared(&self, other: &PyXmuDat, col_x: usize, col_y: usize) -> f64 {
        self.inner.r_squared(&other.inner, col_x, col_y)
    }

    /// Convert to a pandas DataFrame (requires pandas).
    ///
    /// Raises ImportError if pandas is not installed.
    fn to_dataframe(&self, py: Python<'_>) -> PyResult<PyObject> {
        let pd = py.import("pandas")?;
        let dict = pyo3::types::PyDict::new(py);
        for (i, col) in self.inner.columns.iter().enumerate() {
            dict.set_item(format!("col_{i}"), col.as_slice())?;
        }
        let df = pd.call_method1("DataFrame", (dict,))?;
        Ok(df.into())
    }

    /// Access column by index: xmu[0], xmu[1], etc.
    fn __getitem__(&self, index: isize) -> PyResult<Vec<f64>> {
        let ncols = self.inner.columns.len() as isize;
        // Support negative indexing.
        let idx = if index < 0 { ncols + index } else { index };
        if idx < 0 || idx >= ncols {
            return Err(pyo3::exceptions::PyIndexError::new_err(format!(
                "column index {index} out of range (ncols={})",
                self.inner.columns.len()
            )));
        }
        Ok(self.inner.columns[idx as usize].clone())
    }

    /// Iterate over columns.
    fn __iter__(slf: PyRef<'_, Self>) -> PyXmuDatIter {
        PyXmuDatIter {
            columns: slf.inner.columns.clone(),
            index: 0,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "XmuDat(header_lines={}, columns={}, rows={})",
            self.inner.header.len(),
            self.ncols(),
            self.nrows()
        )
    }

    fn __str__(&self) -> String {
        let nrows = self.nrows();
        let ncols = self.ncols();
        if nrows == 0 || ncols == 0 {
            return "XmuDat(empty)".to_string();
        }
        let preview_rows = nrows.min(5);
        let mut lines = Vec::with_capacity(preview_rows + 2);
        lines.push(format!("XmuDat: {ncols} columns, {nrows} rows"));
        for r in 0..preview_rows {
            let row: Vec<String> = self
                .inner
                .columns
                .iter()
                .map(|col| format!("{:>12.5}", col[r]))
                .collect();
            lines.push(row.join(" "));
        }
        if nrows > 5 {
            lines.push(format!("  ... ({} more rows)", nrows - 5));
        }
        lines.join("\n")
    }

    fn __len__(&self) -> usize {
        self.nrows()
    }
}

#[pyclass]
struct PyXmuDatIter {
    columns: Vec<Vec<f64>>,
    index: usize,
}

#[pymethods]
impl PyXmuDatIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<Vec<f64>> {
        if self.index < self.columns.len() {
            let col = self.columns[self.index].clone();
            self.index += 1;
            Some(col)
        } else {
            None
        }
    }
}
