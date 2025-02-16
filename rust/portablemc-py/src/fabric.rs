use std::sync::{Arc, Mutex};

use pyo3::prelude::*;

use portablemc::fabric::{GameVersion, Installer, Loader, LoaderVersion};

use crate::installer::GenericInstaller;


/// Define the `_portablemc.fabric` submodule.
pub(super) fn py_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLoader>()?;
    m.add_class::<PyGameVersion>()?;
    m.add_class::<PyLoaderVersion>()?;
    m.add_class::<PyInstaller>()?;
    Ok(())
}

#[pyclass(name = "Loader", module = "portablemc.fabric", eq)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum PyLoader {
    Fabric,
    Quilt,
    LegacyFabric,
    Babric,
}

impl From<PyLoader> for Loader {
    fn from(value: PyLoader) -> Self {
        match value {
            PyLoader::Fabric => Loader::Fabric,
            PyLoader::Quilt => Loader::Quilt,
            PyLoader::LegacyFabric => Loader::LegacyFabric,
            PyLoader::Babric => Loader::Babric,
        }
    }
}

#[pyclass(name = "GameVersion", module = "portablemc.fabric", eq)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum PyGameVersion {
    Stable,
    Unstable,
}

#[derive(FromPyObject, IntoPyObject)]
enum PyGameVersionUnion {
    Version(PyGameVersion),
    Name(String),
}

impl From<PyGameVersionUnion> for GameVersion {
    fn from(value: PyGameVersionUnion) -> Self {
        match value {
            PyGameVersionUnion::Version(PyGameVersion::Stable) => GameVersion::Stable,
            PyGameVersionUnion::Version(PyGameVersion::Unstable) => GameVersion::Unstable,
            PyGameVersionUnion::Name(name) => GameVersion::Name(name),
        }
    }
}

#[pyclass(name = "LoaderVersion", module = "portablemc.fabric", eq)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum PyLoaderVersion {
    Stable,
    Unstable,
}

#[derive(FromPyObject, IntoPyObject)]
enum PyLoaderVersionUnion {
    Version(PyLoaderVersion),
    Name(String),
}

impl From<PyLoaderVersionUnion> for LoaderVersion {
    fn from(value: PyLoaderVersionUnion) -> Self {
        match value {
            PyLoaderVersionUnion::Version(PyLoaderVersion::Stable) => LoaderVersion::Stable,
            PyLoaderVersionUnion::Version(PyLoaderVersion::Unstable) => LoaderVersion::Unstable,
            PyLoaderVersionUnion::Name(name) => LoaderVersion::Name(name),
        }
    }
}

#[pyclass(name = "Installer", module = "portablemc.fabric", frozen, subclass, extends = crate::mojang::PyInstaller)]
pub(crate) struct PyInstaller(pub(crate) Arc<Mutex<GenericInstaller>>);

#[pymethods]
impl PyInstaller {

    #[new]
    #[pyo3(signature = (loader, game_version = PyGameVersionUnion::Version(PyGameVersion::Stable), loader_version = PyLoaderVersionUnion::Version(PyLoaderVersion::Stable)))]
    fn __new__(loader: PyLoader, game_version: PyGameVersionUnion, loader_version: PyLoaderVersionUnion) -> PyClassInitializer<Self> {

        let inst = Arc::new(Mutex::new(
            GenericInstaller::Fabric(Installer::new(loader.into(), game_version, loader_version))
        ));
        
        PyClassInitializer::from(crate::standard::PyInstaller(Arc::clone(&inst)))
            .add_subclass(crate::mojang::PyInstaller(Arc::clone(&inst)))
            .add_subclass(Self(inst))

    }

    #[getter]
    fn loader(&self) -> PyLoader {
        match self.0.lock().unwrap().fabric().loader() {
            Loader::Fabric => PyLoader::Fabric,
            Loader::Quilt => PyLoader::Quilt,
            Loader::LegacyFabric => PyLoader::LegacyFabric,
            Loader::Babric => PyLoader::Babric,
        }
    }

    #[setter]
    fn set_loader(&self, loader: PyLoader) {
        self.0.lock().unwrap().fabric_mut().set_loader(loader.into());
    }

    #[getter]
    fn game_version(&self) -> PyGameVersionUnion {
        match self.0.lock().unwrap().fabric().game_version() {
            GameVersion::Stable => PyGameVersionUnion::Version(PyGameVersion::Stable),
            GameVersion::Unstable => PyGameVersionUnion::Version(PyGameVersion::Unstable),
            GameVersion::Name(name) => PyGameVersionUnion::Name(name.clone()),
        }
    }

    #[setter]
    fn set_game_version(&self, game_version: PyGameVersionUnion) {
        self.0.lock().unwrap().fabric_mut().set_game_version(game_version);
    }

    #[getter]
    fn loader_version(&self) -> PyLoaderVersionUnion {
        match self.0.lock().unwrap().fabric().loader_version() {
            LoaderVersion::Stable => PyLoaderVersionUnion::Version(PyLoaderVersion::Stable),
            LoaderVersion::Unstable => PyLoaderVersionUnion::Version(PyLoaderVersion::Unstable),
            LoaderVersion::Name(name) => PyLoaderVersionUnion::Name(name.clone()),
        }
    }

    #[setter]
    fn set_loader_version(&self, loader_version: PyLoaderVersionUnion) {
        self.0.lock().unwrap().fabric_mut().set_loader_version(loader_version);
    }

}
