use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use portablemc::mojang::{Installer, Version};
use portablemc::standard::default_main_dir;

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

#[pyclass(name = "Installer", subclass)]
struct PyInstaller {
    inner: Installer,
}

#[pymethods]
impl PyInstaller {

    #[new]
    #[pyo3(signature = (version = PyVersionUnion::Version(PyVersion::Release()), main_dir = None))]
    fn __new__(version: PyVersionUnion, main_dir: Option<&str>) -> PyResult<Self> {

        let main_dir = match main_dir {
            Some(dir) => PathBuf::from(dir.to_string()),
            None => default_main_dir()
                .ok_or_else(|| PyValueError::new_err("no default main directory on your system"))?,
        };

        let version = match version {
            PyVersionUnion::Version(PyVersion::Release()) => Version::Release,
            PyVersionUnion::Version(PyVersion::Snapshot()) => Version::Snapshot,
            PyVersionUnion::Version(PyVersion::Name(name)) |
            PyVersionUnion::Name(name) => Version::Name(name),
        };
        
        Ok(Self {
            inner: Installer::new(version, main_dir),
        })

    }

}
