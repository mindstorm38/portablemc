use std::sync::{Arc, Mutex};

use pyo3::prelude::*;

use portablemc::forge::{Installer, Loader, Version};

use crate::installer::GenericInstaller;


/// Define the `_portablemc.forge` submodule.
pub(super) fn py_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLoader>()?;
    m.add_class::<PyVersion>()?;
    m.add_class::<PyInstaller>()?;
    Ok(())
}

#[pyclass(name = "Loader", module = "portablemc.forge", eq)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum PyLoader {
    Forge,
    NeoForge,
}

impl From<PyLoader> for Loader {
    fn from(value: PyLoader) -> Self {
        match value {
            PyLoader::Forge => Loader::Forge,
            PyLoader::NeoForge => Loader::NeoForge,
        }
    }
}

#[pyclass(name = "Version", module = "portablemc.forge", eq)]
#[derive(Clone, PartialEq, Eq)]
enum PyVersion {
    Stable(String),
    Unstable(String),
    Name(String),
}

impl From<PyVersion> for Version {
    fn from(value: PyVersion) -> Self {
        match value {
            PyVersion::Stable(game_version) => Version::Stable(game_version),
            PyVersion::Unstable(game_version) => Version::Unstable(game_version),
            PyVersion::Name(name) => Version::Name(name),
        }
    }
}

#[pyclass(name = "Installer", module = "portablemc.forge", frozen, subclass, extends = crate::mojang::PyInstaller)]
pub(crate) struct PyInstaller(pub(crate) Arc<Mutex<GenericInstaller>>);

#[pymethods]
impl PyInstaller {

    #[new]
    fn __new__(loader: PyLoader, version: PyVersion) -> PyClassInitializer<Self> {

        let inst = Arc::new(Mutex::new(
            GenericInstaller::Forge(Installer::new(loader.into(), version))
        ));
        
        PyClassInitializer::from(crate::standard::PyInstaller(Arc::clone(&inst)))
            .add_subclass(crate::mojang::PyInstaller(Arc::clone(&inst)))
            .add_subclass(Self(inst))

    }

    fn __repr__(&self) -> String {
        let guard = self.0.lock().unwrap();
        let inst = guard.forge();
        format!("<portablemc.forge.Installer loader=Loader.{:?} version=Version.{:?}>", inst.loader(), inst.version())
    }

    #[getter]
    fn loader(&self) -> PyLoader {
        match self.0.lock().unwrap().forge().loader() {
            Loader::Forge => PyLoader::Forge,
            Loader::NeoForge => PyLoader::NeoForge,
        }
    }

    #[setter]
    fn set_loader(&self, loader: PyLoader) {
        self.0.lock().unwrap().forge_mut().set_loader(loader.into());
    }

    #[getter]
    fn version(&self) -> PyVersion {
        match self.0.lock().unwrap().forge().version() {
            Version::Stable(game_version) => PyVersion::Stable(game_version.clone()),
            Version::Unstable(game_version) => PyVersion::Unstable(game_version.clone()),
            Version::Name(name) => PyVersion::Name(name.clone()),
        }
    }

    #[setter]
    fn set_version(&self, version: PyVersion) {
        self.0.lock().unwrap().forge_mut().set_version(version);
    }

}
