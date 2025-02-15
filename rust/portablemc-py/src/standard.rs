use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pyo3::prelude::*;

use portablemc::standard::{Installer, JvmPolicy, default_main_dir};

use crate::installer::GenericInstaller;


/// Define the `_portablemc.standard` submodule.
pub(super) fn py_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyJvmPolicy>()?;
    m.add_class::<PyInstaller>()?;
    m.add_function(wrap_pyfunction!(py_default_main_dir, m)?)?;
    Ok(())
}

#[pyfunction]
#[pyo3(name = "default_main_dir")]
fn py_default_main_dir() -> Option<&'static Path> {
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

#[pyclass(name = "Installer", frozen, subclass)]
pub(crate) struct PyInstaller(pub(crate) Arc<Mutex<GenericInstaller>>);

#[pymethods]
impl PyInstaller {

    #[new]
    fn __new__(version: &str) -> PyResult<Self> {
        
        let inst = Arc::new(Mutex::new(
            GenericInstaller::Standard(Installer::new(version.to_string()))
        ));

        Ok(Self(inst))

    }

    #[getter]
    fn version(&self) -> String {
        self.0.lock().unwrap().standard().version().to_string()
    }

    #[setter]
    fn set_version(&self, version: String) {
        self.0.lock().unwrap().standard_mut().set_version(version);
    }

    #[getter]
    fn versions_dir(&self) -> PathBuf {
        self.0.lock().unwrap().standard().versions_dir().to_path_buf()
    }

    #[setter]
    fn set_versions_dir(&self, dir: PathBuf) {
        self.0.lock().unwrap().standard_mut().set_versions_dir(dir);
    }

    #[getter]
    fn libraries_dir(&self) -> PathBuf {
        self.0.lock().unwrap().standard().libraries_dir().to_path_buf()
    }

    #[setter]
    fn set_libraries_dir(&self, dir: PathBuf) {
        self.0.lock().unwrap().standard_mut().set_libraries_dir(dir);
    }

    #[getter]
    fn assets_dir(&self) -> PathBuf {
        self.0.lock().unwrap().standard().assets_dir().to_path_buf()
    }

    #[setter]
    fn set_assets_dir(&self, dir: PathBuf) {
        self.0.lock().unwrap().standard_mut().set_assets_dir(dir);
    }

    #[getter]
    fn jvm_dir(&self) -> PathBuf {
        self.0.lock().unwrap().standard().jvm_dir().to_path_buf()
    }

    #[setter]
    fn set_jvm_dir(&self, dir: PathBuf) {
        self.0.lock().unwrap().standard_mut().set_jvm_dir(dir);
    }

    #[getter]
    fn bin_dir(&self) -> PathBuf {
        self.0.lock().unwrap().standard().bin_dir().to_path_buf()
    }

    #[setter]
    fn set_bin_dir(&self, dir: PathBuf) {
        self.0.lock().unwrap().standard_mut().set_bin_dir(dir);
    }

    #[getter]
    fn mc_dir(&self) -> PathBuf {
        self.0.lock().unwrap().standard().mc_dir().to_path_buf()
    }

    #[setter]
    fn set_mc_dir(&self, dir: PathBuf) {
        self.0.lock().unwrap().standard_mut().set_mc_dir(dir);
    }

    // No setter because it's a compound function, setting all paths below.
    fn set_main_dir(&self, dir: PathBuf) {
        self.0.lock().unwrap().standard_mut().set_main_dir(dir);
    }

    #[getter]
    fn strict_assets_check(&self) -> bool {
        self.0.lock().unwrap().standard().strict_assets_check()
    }

    #[setter]
    fn set_strict_assets_check(&self, strict: bool) {
        self.0.lock().unwrap().standard_mut().set_strict_assets_check(strict);
    }

    #[getter]
    fn strict_libraries_check(&self) -> bool {
        self.0.lock().unwrap().standard().strict_libraries_check()
    }

    #[setter]
    fn set_strict_libraries_check(&self, strict: bool) {
        self.0.lock().unwrap().standard_mut().set_strict_libraries_check(strict);
    }

    #[getter]
    fn strict_jvm_check(&self) -> bool {
        self.0.lock().unwrap().standard().strict_jvm_check()
    }

    #[setter]
    fn set_strict_jvm_check(&self, strict: bool) {
        self.0.lock().unwrap().standard_mut().set_strict_jvm_check(strict);
    }

    #[getter]
    fn jvm_policy(&self) -> PyJvmPolicyUnion {
        match self.0.lock().unwrap().standard().jvm_policy() {
            JvmPolicy::Static(file) => PyJvmPolicyUnion::Static(file.clone()),
            JvmPolicy::System => PyJvmPolicyUnion::Policy(PyJvmPolicy::System),
            JvmPolicy::Mojang => PyJvmPolicyUnion::Policy(PyJvmPolicy::Mojang),
            JvmPolicy::SystemThenMojang => PyJvmPolicyUnion::Policy(PyJvmPolicy::SystemThenMojang),
            JvmPolicy::MojangThenSystem => PyJvmPolicyUnion::Policy(PyJvmPolicy::MojangThenSystem),
        }
    }

    #[setter]
    fn set_jvm_policy(&self, policy: PyJvmPolicyUnion) {
        self.0.lock().unwrap().standard_mut().set_jvm_policy(match policy {
            PyJvmPolicyUnion::Static(file) => JvmPolicy::Static(file),
            PyJvmPolicyUnion::Policy(PyJvmPolicy::System) => JvmPolicy::System,
            PyJvmPolicyUnion::Policy(PyJvmPolicy::Mojang) => JvmPolicy::Mojang,
            PyJvmPolicyUnion::Policy(PyJvmPolicy::SystemThenMojang) => JvmPolicy::SystemThenMojang,
            PyJvmPolicyUnion::Policy(PyJvmPolicy::MojangThenSystem) => JvmPolicy::MojangThenSystem,
        });
    }

    #[getter]
    fn launcher_name(&self) -> String {
        self.0.lock().unwrap().standard().launcher_name().to_string()
    }

    #[setter]
    fn set_launcher_name(&self, name: String) {
        self.0.lock().unwrap().standard_mut().set_launcher_name(name);
    }

    #[getter]
    fn launcher_version(&self) -> String {
        self.0.lock().unwrap().standard().launcher_version().to_string()
    }
    
    #[setter]
    fn set_launcher_version(&self, version: String) {
        self.0.lock().unwrap().standard_mut().set_launcher_version(version);
    }

}
