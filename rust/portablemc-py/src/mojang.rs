use std::sync::{Arc, Mutex};

use pyo3::prelude::*;

use portablemc::mojang::{Installer, Version};

use crate::installer::GenericInstaller;


/// Define the `_portablemc.mojang` submodule.
pub(super) fn py_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyVersion>()?;
    m.add_class::<PyInstaller>()?;
    Ok(())
}

#[pyclass(eq, name = "Version")]
#[derive(Clone, PartialEq, Eq)]
enum PyVersion {
    Release(),
    Snapshot(),
    Name(String),
}

#[derive(FromPyObject)]
enum PyVersionUnion {
    Version(PyVersion),
    Name(String),
}

impl From<PyVersionUnion> for Version {
    fn from(value: PyVersionUnion) -> Self {
        match value {
            PyVersionUnion::Version(PyVersion::Release()) => Version::Release,
            PyVersionUnion::Version(PyVersion::Snapshot()) => Version::Snapshot,
            PyVersionUnion::Version(PyVersion::Name(name)) |
            PyVersionUnion::Name(name) => Version::Name(name),
        }
    }
}

#[pyclass(name = "Installer", frozen, subclass, extends = crate::standard::PyInstaller)]
pub(crate) struct PyInstaller(pub(crate) Arc<Mutex<GenericInstaller>>);

#[pymethods]
impl PyInstaller {

    #[new]
    #[pyo3(signature = (version = PyVersionUnion::Version(PyVersion::Release())))]
    fn __new__(version: PyVersionUnion) -> PyClassInitializer<Self> {

        let inst = Arc::new(Mutex::new(
            GenericInstaller::Mojang(Installer::new(version))
        ));
        
        PyClassInitializer::from(crate::standard::PyInstaller(Arc::clone(&inst)))
            .add_subclass(Self(inst))

    }

    #[getter]
    fn version(&self) -> PyVersion {
        match self.0.lock().unwrap().mojang().version() {
            Version::Release => PyVersion::Release(),
            Version::Snapshot => PyVersion::Snapshot(),
            Version::Name(name) => PyVersion::Name(name.clone()),
        }
    }

    #[setter]
    fn set_version(&self, version: PyVersionUnion) {
        self.0.lock().unwrap().mojang_mut().set_version(version);
    }

}
