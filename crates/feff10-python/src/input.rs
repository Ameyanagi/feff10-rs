use feff10::input::{Atom, FeffInput, Potential};
use pyo3::prelude::*;

use crate::error::to_pyerr;

#[pyclass(name = "Potential", from_py_object)]
#[derive(Clone)]
pub struct PyPotential {
    pub(crate) inner: Potential,
}

#[pymethods]
impl PyPotential {
    #[new]
    #[pyo3(signature = (ipot, z, tag, l_scmt=None, l_fms=None, stoich=None))]
    fn new(
        ipot: u32,
        z: u32,
        tag: String,
        l_scmt: Option<u32>,
        l_fms: Option<u32>,
        stoich: Option<f64>,
    ) -> Self {
        Self {
            inner: Potential {
                ipot,
                z,
                tag,
                l_scmt,
                l_fms,
                stoich,
            },
        }
    }

    #[getter]
    fn ipot(&self) -> u32 {
        self.inner.ipot
    }
    #[setter]
    fn set_ipot(&mut self, v: u32) {
        self.inner.ipot = v;
    }
    #[getter]
    fn z(&self) -> u32 {
        self.inner.z
    }
    #[setter]
    fn set_z(&mut self, v: u32) {
        self.inner.z = v;
    }
    #[getter]
    fn tag(&self) -> &str {
        &self.inner.tag
    }
    #[setter]
    fn set_tag(&mut self, v: String) {
        self.inner.tag = v;
    }
    #[getter]
    fn l_scmt(&self) -> Option<u32> {
        self.inner.l_scmt
    }
    #[setter]
    fn set_l_scmt(&mut self, v: Option<u32>) {
        self.inner.l_scmt = v;
    }
    #[getter]
    fn l_fms(&self) -> Option<u32> {
        self.inner.l_fms
    }
    #[setter]
    fn set_l_fms(&mut self, v: Option<u32>) {
        self.inner.l_fms = v;
    }
    #[getter]
    fn stoich(&self) -> Option<f64> {
        self.inner.stoich
    }
    #[setter]
    fn set_stoich(&mut self, v: Option<f64>) {
        self.inner.stoich = v;
    }

    fn __repr__(&self) -> String {
        format!(
            "Potential(ipot={}, z={}, tag='{}')",
            self.inner.ipot, self.inner.z, self.inner.tag
        )
    }

    fn __str__(&self) -> String {
        format!(
            "ipot={} Z={} {}",
            self.inner.ipot, self.inner.z, self.inner.tag
        )
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner.ipot == other.inner.ipot
            && self.inner.z == other.inner.z
            && self.inner.tag == other.inner.tag
            && self.inner.l_scmt == other.inner.l_scmt
            && self.inner.l_fms == other.inner.l_fms
            && self.inner.stoich == other.inner.stoich
    }
}

#[pyclass(name = "Atom", from_py_object)]
#[derive(Clone)]
pub struct PyAtom {
    pub(crate) inner: Atom,
}

#[pymethods]
impl PyAtom {
    #[new]
    #[pyo3(signature = (x, y, z, ipot, tag, distance=0.0))]
    fn new(x: f64, y: f64, z: f64, ipot: u32, tag: String, distance: f64) -> Self {
        Self {
            inner: Atom {
                x,
                y,
                z,
                ipot,
                tag,
                distance,
            },
        }
    }

    #[getter]
    fn x(&self) -> f64 {
        self.inner.x
    }
    #[setter]
    fn set_x(&mut self, v: f64) {
        self.inner.x = v;
    }
    #[getter]
    fn y(&self) -> f64 {
        self.inner.y
    }
    #[setter]
    fn set_y(&mut self, v: f64) {
        self.inner.y = v;
    }
    #[getter]
    fn z(&self) -> f64 {
        self.inner.z
    }
    #[setter]
    fn set_z(&mut self, v: f64) {
        self.inner.z = v;
    }
    #[getter]
    fn ipot(&self) -> u32 {
        self.inner.ipot
    }
    #[setter]
    fn set_ipot(&mut self, v: u32) {
        self.inner.ipot = v;
    }
    #[getter]
    fn tag(&self) -> &str {
        &self.inner.tag
    }
    #[setter]
    fn set_tag(&mut self, v: String) {
        self.inner.tag = v;
    }
    #[getter]
    fn distance(&self) -> f64 {
        self.inner.distance
    }
    #[setter]
    fn set_distance(&mut self, v: f64) {
        self.inner.distance = v;
    }

    fn __repr__(&self) -> String {
        format!(
            "Atom(x={}, y={}, z={}, ipot={}, tag='{}')",
            self.inner.x, self.inner.y, self.inner.z, self.inner.ipot, self.inner.tag
        )
    }

    fn __str__(&self) -> String {
        format!(
            "{:>10.5} {:>10.5} {:>10.5}  {} {}",
            self.inner.x, self.inner.y, self.inner.z, self.inner.ipot, self.inner.tag
        )
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner.x == other.inner.x
            && self.inner.y == other.inner.y
            && self.inner.z == other.inner.z
            && self.inner.ipot == other.inner.ipot
            && self.inner.tag == other.inner.tag
            && self.inner.distance == other.inner.distance
    }
}

#[pyclass(name = "FeffInput", from_py_object)]
#[derive(Clone)]
pub struct PyFeffInput {
    pub(crate) inner: FeffInput,
}

#[pymethods]
impl PyFeffInput {
    #[new]
    #[pyo3(signature = (
        title=vec![],
        edge=None,
        s02=None,
        control=None,
        print_flags=None,
        potentials=vec![],
        atoms=vec![],
        other_cards=vec![],
    ))]
    fn new(
        title: Vec<String>,
        edge: Option<String>,
        s02: Option<f64>,
        control: Option<[u32; 6]>,
        print_flags: Option<[u32; 6]>,
        potentials: Vec<PyPotential>,
        atoms: Vec<PyAtom>,
        other_cards: Vec<String>,
    ) -> Self {
        Self {
            inner: FeffInput {
                title,
                edge,
                s02,
                control: control.unwrap_or([1; 6]),
                print_flags: print_flags.unwrap_or([0; 6]),
                potentials: potentials.into_iter().map(|p| p.inner).collect(),
                atoms: atoms.into_iter().map(|a| a.inner).collect(),
                other_cards,
            },
        }
    }

    /// Parse from a string.
    #[staticmethod]
    fn parse(content: &str) -> PyResult<Self> {
        FeffInput::parse(content)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Parse from a string with strict validation.
    #[staticmethod]
    fn parse_strict(content: &str) -> PyResult<Self> {
        FeffInput::parse_strict(content)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Parse from a file path.
    #[staticmethod]
    fn from_file(path: &str) -> PyResult<Self> {
        FeffInput::from_file(path)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Parse from a file path with strict validation.
    #[staticmethod]
    fn from_file_strict(path: &str) -> PyResult<Self> {
        FeffInput::from_file_strict(path)
            .map(|inner| Self { inner })
            .map_err(to_pyerr)
    }

    /// Write the input to a string (returns feff.inp content).
    fn to_string(&self) -> PyResult<String> {
        let mut buf = Vec::new();
        self.inner
            .write_to(&mut buf)
            .map_err(|e| to_pyerr(feff10::error::Error::Io(e)))?;
        Ok(String::from_utf8(buf).unwrap())
    }

    /// Write to a file path.
    fn write_to_file(&self, path: &str) -> PyResult<()> {
        let mut f =
            std::fs::File::create(path).map_err(|e| to_pyerr(feff10::error::Error::Io(e)))?;
        self.inner
            .write_to(&mut f)
            .map_err(|e| to_pyerr(feff10::error::Error::Io(e)))
    }

    #[getter]
    fn title(&self) -> Vec<String> {
        self.inner.title.clone()
    }
    #[setter]
    fn set_title(&mut self, v: Vec<String>) {
        self.inner.title = v;
    }
    #[getter]
    fn edge(&self) -> Option<&str> {
        self.inner.edge.as_deref()
    }
    #[setter]
    fn set_edge(&mut self, v: Option<String>) {
        self.inner.edge = v;
    }
    #[getter]
    fn s02(&self) -> Option<f64> {
        self.inner.s02
    }
    #[setter]
    fn set_s02(&mut self, v: Option<f64>) {
        self.inner.s02 = v;
    }
    #[getter]
    fn control(&self) -> [u32; 6] {
        self.inner.control
    }
    #[setter]
    fn set_control(&mut self, v: [u32; 6]) {
        self.inner.control = v;
    }
    #[getter]
    fn print_flags(&self) -> [u32; 6] {
        self.inner.print_flags
    }
    #[setter]
    fn set_print_flags(&mut self, v: [u32; 6]) {
        self.inner.print_flags = v;
    }
    #[getter]
    fn other_cards(&self) -> Vec<String> {
        self.inner.other_cards.clone()
    }
    #[setter]
    fn set_other_cards(&mut self, v: Vec<String>) {
        self.inner.other_cards = v;
    }

    /// Number of potentials.
    #[getter]
    fn num_potentials(&self) -> usize {
        self.inner.potentials.len()
    }

    /// Number of atoms.
    #[getter]
    fn num_atoms(&self) -> usize {
        self.inner.atoms.len()
    }

    #[getter]
    fn potentials(&self) -> Vec<PyPotential> {
        self.inner
            .potentials
            .iter()
            .map(|p| PyPotential { inner: p.clone() })
            .collect()
    }
    #[setter]
    fn set_potentials(&mut self, v: Vec<PyPotential>) {
        self.inner.potentials = v.into_iter().map(|p| p.inner).collect();
    }

    #[getter]
    fn atoms(&self) -> Vec<PyAtom> {
        self.inner
            .atoms
            .iter()
            .map(|a| PyAtom { inner: a.clone() })
            .collect()
    }
    #[setter]
    fn set_atoms(&mut self, v: Vec<PyAtom>) {
        self.inner.atoms = v.into_iter().map(|a| a.inner).collect();
    }

    fn __repr__(&self) -> String {
        format!(
            "FeffInput(edge={:?}, potentials={}, atoms={})",
            self.inner.edge,
            self.inner.potentials.len(),
            self.inner.atoms.len(),
        )
    }

    fn __str__(&self) -> PyResult<String> {
        self.to_string()
    }
}
