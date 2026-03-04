use std::path::Path;

use feff10::output::{FeffOutputs, FeffTable, OutputKind, PathsDat};
use pyo3::prelude::*;

use crate::error::to_pyerr;

#[pyclass(name = "FeffTable", from_py_object)]
#[derive(Clone)]
pub struct PyFeffTable {
    pub(crate) inner: FeffTable,
}

#[pymethods]
impl PyFeffTable {
    /// Parse FEFF table content from a string.
    #[staticmethod]
    fn parse(content: &str) -> PyResult<Self> {
        FeffTable::parse(content)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Parse FEFF table content from a string with strict validation.
    #[staticmethod]
    fn parse_strict(content: &str) -> PyResult<Self> {
        FeffTable::parse_strict(content)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Parse from a file path.
    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        FeffTable::from_file(path)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Parse from a file path with strict validation.
    #[staticmethod]
    fn from_file_strict(path: &str) -> PyResult<Self> {
        FeffTable::from_file_strict(path)
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
        self.inner.ncols()
    }

    /// Number of data points (rows).
    #[getter]
    fn nrows(&self) -> usize {
        self.inner.nrows()
    }

    /// Get a specific column by index.
    fn column(&self, index: usize) -> PyResult<Vec<f64>> {
        self.inner.columns.get(index).cloned().ok_or_else(|| {
            pyo3::exceptions::PyIndexError::new_err(format!(
                "column index {index} out of range (ncols={})",
                self.inner.columns.len()
            ))
        })
    }

    /// Compare with another FeffTable using R-squared metric.
    fn r_squared(&self, other: &PyFeffTable, col_x: usize, col_y: usize) -> f64 {
        self.inner.r_squared(&other.inner, col_x, col_y)
    }

    /// Convert to a pandas DataFrame (requires pandas).
    fn to_dataframe(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let pd = py.import("pandas")?;
        let dict = pyo3::types::PyDict::new(py);
        for (i, col) in self.inner.columns.iter().enumerate() {
            dict.set_item(format!("col_{i}"), col.as_slice())?;
        }
        let df = pd.call_method1("DataFrame", (dict,))?;
        Ok(df.into())
    }

    /// Access column by index: table[0], table[1], etc.
    fn __getitem__(&self, index: isize) -> PyResult<Vec<f64>> {
        let ncols = self.inner.columns.len() as isize;
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
    fn __iter__(slf: PyRef<'_, Self>) -> PyFeffTableIter {
        PyFeffTableIter {
            columns: slf.inner.columns.clone(),
            index: 0,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "FeffTable(header_lines={}, columns={}, rows={})",
            self.inner.header.len(),
            self.ncols(),
            self.nrows()
        )
    }

    fn __str__(&self) -> String {
        let nrows = self.nrows();
        let ncols = self.ncols();
        if nrows == 0 || ncols == 0 {
            return "FeffTable(empty)".to_string();
        }
        let preview_rows = nrows.min(5);
        let mut lines = Vec::with_capacity(preview_rows + 2);
        lines.push(format!("FeffTable: {ncols} columns, {nrows} rows"));
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
struct PyFeffTableIter {
    columns: Vec<Vec<f64>>,
    index: usize,
}

#[pymethods]
impl PyFeffTableIter {
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

#[pyclass(name = "PathLeg", from_py_object)]
#[derive(Clone)]
pub struct PyPathLeg {
    #[pyo3(get)]
    x: f64,
    #[pyo3(get)]
    y: f64,
    #[pyo3(get)]
    z: f64,
    #[pyo3(get)]
    ipot: i32,
    #[pyo3(get)]
    label: String,
    #[pyo3(get)]
    rleg: f64,
    #[pyo3(get)]
    beta: f64,
    #[pyo3(get)]
    eta: f64,
}

impl From<&feff10::output::PathLeg> for PyPathLeg {
    fn from(v: &feff10::output::PathLeg) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
            ipot: v.ipot,
            label: v.label.clone(),
            rleg: v.rleg,
            beta: v.beta,
            eta: v.eta,
        }
    }
}

#[pymethods]
impl PyPathLeg {
    fn __repr__(&self) -> String {
        format!(
            "PathLeg(x={}, y={}, z={}, ipot={}, label='{}')",
            self.x, self.y, self.z, self.ipot, self.label
        )
    }
}

#[pyclass(name = "PathEntry", from_py_object)]
#[derive(Clone)]
pub struct PyPathEntry {
    #[pyo3(get)]
    index: u32,
    #[pyo3(get)]
    nleg: usize,
    #[pyo3(get)]
    degeneracy: f64,
    #[pyo3(get)]
    r: f64,
    #[pyo3(get)]
    legs: Vec<PyPathLeg>,
}

impl From<&feff10::output::PathEntry> for PyPathEntry {
    fn from(v: &feff10::output::PathEntry) -> Self {
        Self {
            index: v.index,
            nleg: v.nleg,
            degeneracy: v.degeneracy,
            r: v.r,
            legs: v.legs.iter().map(PyPathLeg::from).collect(),
        }
    }
}

#[pymethods]
impl PyPathEntry {
    fn __repr__(&self) -> String {
        format!(
            "PathEntry(index={}, nleg={}, degeneracy={}, r={})",
            self.index, self.nleg, self.degeneracy, self.r
        )
    }
}

#[pyclass(name = "PathsDat", from_py_object)]
#[derive(Clone)]
pub struct PyPathsDat {
    pub(crate) inner: PathsDat,
}

#[pymethods]
impl PyPathsDat {
    #[staticmethod]
    fn parse(content: &str) -> PyResult<Self> {
        PathsDat::parse(content)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        PathsDat::from_file(path)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    #[getter]
    fn header(&self) -> Vec<String> {
        self.inner.header.clone()
    }

    #[getter]
    fn entries(&self) -> Vec<PyPathEntry> {
        self.inner.entries.iter().map(PyPathEntry::from).collect()
    }

    #[getter]
    fn npaths(&self) -> usize {
        self.inner.len()
    }

    fn total_degeneracy(&self) -> f64 {
        self.inner.total_degeneracy()
    }

    fn max_r(&self) -> Option<f64> {
        self.inner.max_r()
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }

    fn __repr__(&self) -> String {
        format!("PathsDat(paths={})", self.inner.len())
    }
}

#[pyclass(name = "OutputFileInfo", from_py_object)]
#[derive(Clone)]
pub struct PyOutputFileInfo {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    path: String,
    #[pyo3(get)]
    kind: String,
}

#[pymethods]
impl PyOutputFileInfo {
    fn __repr__(&self) -> String {
        format!(
            "OutputFileInfo(name='{}', kind='{}', path='{}')",
            self.name, self.kind, self.path
        )
    }
}

#[pyclass(name = "FeffOutputs", from_py_object)]
#[derive(Clone)]
pub struct PyFeffOutputs {
    inner: FeffOutputs,
}

impl PyFeffOutputs {
    pub(crate) fn from_rust(inner: FeffOutputs) -> Self {
        Self { inner }
    }
}

#[pymethods]
impl PyFeffOutputs {
    #[staticmethod]
    fn discover(work_dir: &str) -> PyResult<Self> {
        FeffOutputs::discover(work_dir)
            .map(Self::from_rust)
            .map_err(to_pyerr)
    }

    #[getter]
    fn work_dir(&self) -> String {
        self.inner.work_dir.display().to_string()
    }

    #[getter]
    fn files(&self) -> Vec<PyOutputFileInfo> {
        self.inner
            .files
            .iter()
            .map(|f| PyOutputFileInfo {
                name: f.name.clone(),
                path: f.path.display().to_string(),
                kind: output_kind_to_str(f.kind).to_string(),
            })
            .collect()
    }

    #[pyo3(signature = (name, strict=false))]
    fn read_table(&self, name: &str, strict: bool) -> PyResult<PyFeffTable> {
        let table = if strict {
            self.inner.read_table_strict(name)
        } else {
            self.inner.read_table(name)
        };
        table
            .map(|inner| PyFeffTable { inner })
            .map_err(to_pyerr)
    }

    fn read_paths(&self) -> PyResult<PyPathsDat> {
        self.inner
            .read_paths()
            .map(|inner| PyPathsDat { inner })
            .map_err(to_pyerr)
    }

    #[pyo3(signature = (strict=false))]
    fn read_xmu(&self, strict: bool) -> PyResult<PyFeffTable> {
        let table = if strict {
            self.inner.read_xmu_strict()
        } else {
            self.inner.read_xmu()
        };
        table
            .map(|inner| PyFeffTable { inner })
            .map_err(to_pyerr)
    }

    #[pyo3(signature = (strict=false))]
    fn read_chi(&self, strict: bool) -> PyResult<PyFeffTable> {
        let table = if strict {
            self.inner.read_chi_strict()
        } else {
            self.inner.read_chi()
        };
        table
            .map(|inner| PyFeffTable { inner })
            .map_err(to_pyerr)
    }

    #[pyo3(signature = (strict=false))]
    fn read_eels(&self, strict: bool) -> PyResult<PyFeffTable> {
        let table = if strict {
            self.inner.read_eels_strict()
        } else {
            self.inner.read_eels()
        };
        table
            .map(|inner| PyFeffTable { inner })
            .map_err(to_pyerr)
    }

    #[pyo3(signature = (index, strict=false))]
    fn read_ldos(&self, index: u32, strict: bool) -> PyResult<PyFeffTable> {
        let table = if strict {
            self.inner.read_ldos_strict(index)
        } else {
            self.inner.read_ldos(index)
        };
        table
            .map(|inner| PyFeffTable { inner })
            .map_err(to_pyerr)
    }

    fn __len__(&self) -> usize {
        self.inner.files.len()
    }

    fn __repr__(&self) -> String {
        format!(
            "FeffOutputs(work_dir='{}', files={})",
            self.inner.work_dir.display(),
            self.inner.files.len()
        )
    }
}

pub(crate) fn discover_outputs_from_work_dir(work_dir: &str) -> PyResult<PyFeffOutputs> {
    FeffOutputs::discover(Path::new(work_dir))
        .map(PyFeffOutputs::from_rust)
        .map_err(to_pyerr)
}

fn output_kind_to_str(kind: OutputKind) -> &'static str {
    match kind {
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
