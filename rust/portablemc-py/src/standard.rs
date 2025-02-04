use std::path::{Path, PathBuf};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use portablemc::standard::{Installer, JvmPolicy, default_main_dir};


/// Define the `_portablemc.standard` submodule.
pub(super) fn py_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyJvmPolicy>()?;
    m.add_class::<PyInstaller>()?;
    m.add_function(wrap_pyfunction!(py_default_main_dir, m)?)?;
    Ok(())
}

#[pyfunction]
#[pyo3(name = "default_main_dir")]
fn py_default_main_dir() -> Option<PathBuf> {
    default_main_dir()
}

#[pyclass(name = "JvmPolicy", eq)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum PyJvmPolicy {
    System,
    Mojang,
    SystemThenMojang,
    MojangThenSystem,
}

#[derive(FromPyObject, IntoPyObject)]
enum PyJvmPolicyUnion {
    Static(PathBuf),
    Policy(PyJvmPolicy),
}

#[pyclass(name = "Installer", subclass)]
pub(crate) struct PyInstaller {
    inner: Installer,
}

#[pymethods]
impl PyInstaller {

    #[new]
    #[pyo3(signature = (version, main_dir = None))]
    fn __new__(version: &str, main_dir: Option<&str>) -> PyResult<Self> {

        let main_dir = match main_dir {
            Some(dir) => PathBuf::from(dir.to_string()),
            None => default_main_dir()
                .ok_or_else(|| PyValueError::new_err("no default main directory on your system"))?,
        };

        Ok(Self {
            inner: Installer::new(version.to_string(), main_dir),
        })

    }

    #[getter]
    fn version(&self) -> &str {
        self.inner.version()
    }

    #[setter]
    fn set_version(&mut self, version: String) {
        self.inner.set_version(version);
    }

    // No setter because it's a compound function, setting all paths below.
    fn set_main_dir(&mut self, dir: PathBuf) {
        self.inner.set_main_dir(dir);
    }

    #[getter]
    fn versions_dir(&self) -> &Path {
        self.inner.versions_dir()
    }

    #[setter]
    fn set_versions_dir(&mut self, dir: PathBuf) {
        self.inner.set_versions_dir(dir);
    }

    #[getter]
    fn libraries_dir(&self) -> &Path {
        self.inner.libraries_dir()
    }

    #[setter]
    fn set_libraries_dir(&mut self, dir: PathBuf) {
        self.inner.set_libraries_dir(dir);
    }

    #[getter]
    fn assets_dir(&self) -> &Path {
        self.inner.assets_dir()
    }

    #[setter]
    fn set_assets_dir(&mut self, dir: PathBuf) {
        self.inner.set_assets_dir(dir);
    }

    #[getter]
    fn jvm_dir(&self) -> &Path {
        self.inner.jvm_dir()
    }

    #[setter]
    fn set_jvm_dir(&mut self, dir: PathBuf) {
        self.inner.set_jvm_dir(dir);
    }

    #[getter]
    fn bin_dir(&self) -> &Path {
        self.inner.bin_dir()
    }

    #[setter]
    fn set_bin_dir(&mut self, dir: PathBuf) {
        self.inner.set_bin_dir(dir);
    }

    #[getter]
    fn mc_dir(&self) -> &Path {
        self.inner.mc_dir()
    }

    #[setter]
    fn set_mc_dir(&mut self, dir: PathBuf) {
        self.inner.set_mc_dir(dir);
    }

    #[getter]
    fn strict_assets_check(&self) -> bool {
        self.inner.strict_assets_check()
    }

    #[setter]
    fn set_strict_assets_check(&mut self, strict: bool) {
        self.inner.set_strict_assets_check(strict);
    }

    #[getter]
    fn strict_libraries_check(&self) -> bool {
        self.inner.strict_libraries_check()
    }

    #[setter]
    fn set_strict_libraries_check(&mut self, strict: bool) {
        self.inner.set_strict_libraries_check(strict);
    }

    #[getter]
    fn strict_jvm_check(&self) -> bool {
        self.inner.strict_jvm_check()
    }

    #[setter]
    fn set_strict_jvm_check(&mut self, strict: bool) {
        self.inner.set_strict_jvm_check(strict);
    }

    #[getter]
    fn jvm_policy(&self) -> PyJvmPolicyUnion {
        match self.inner.jvm_policy() {
            JvmPolicy::Static(file) => PyJvmPolicyUnion::Static(file.clone()),
            JvmPolicy::System => PyJvmPolicyUnion::Policy(PyJvmPolicy::System),
            JvmPolicy::Mojang => PyJvmPolicyUnion::Policy(PyJvmPolicy::Mojang),
            JvmPolicy::SystemThenMojang => PyJvmPolicyUnion::Policy(PyJvmPolicy::SystemThenMojang),
            JvmPolicy::MojangThenSystem => PyJvmPolicyUnion::Policy(PyJvmPolicy::MojangThenSystem),
        }
    }

    #[setter]
    fn set_jvm_policy(&mut self, policy: PyJvmPolicyUnion) {
        self.inner.set_jvm_policy(match policy {
            PyJvmPolicyUnion::Static(file) => JvmPolicy::Static(file),
            PyJvmPolicyUnion::Policy(PyJvmPolicy::System) => JvmPolicy::System,
            PyJvmPolicyUnion::Policy(PyJvmPolicy::Mojang) => JvmPolicy::Mojang,
            PyJvmPolicyUnion::Policy(PyJvmPolicy::SystemThenMojang) => JvmPolicy::SystemThenMojang,
            PyJvmPolicyUnion::Policy(PyJvmPolicy::MojangThenSystem) => JvmPolicy::MojangThenSystem,
        });
    }

    #[getter]
    fn launcher_name(&self) -> &str {
        self.inner.launcher_name()
    }

    #[setter]
    fn set_launcher_name(&mut self, name: String) {
        self.inner.set_launcher_name(name);
    }

    #[getter]
    fn launcher_version(&self) -> &str {
        self.inner.launcher_version()
    }
    
    #[setter]
    fn set_launcher_version(&mut self, version: String) {
        self.inner.set_launcher_version(version);
    }

}
