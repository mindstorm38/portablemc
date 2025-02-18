use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use pyo3::types::{IntoPyDict, PyList};
use pyo3::{intern, prelude::*};

use portablemc::standard::{default_main_dir, Installer, Game, JvmPolicy};

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

#[pyclass(name = "JvmPolicy", module = "portablemc.standard", eq)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PyJvmPolicy {
    System,
    Mojang,
    SystemThenMojang,
    MojangThenSystem,
}

#[derive(FromPyObject, IntoPyObject)]
pub enum PyJvmPolicyUnion {
    Static(PathBuf),
    Policy(PyJvmPolicy),
}

#[pyclass(name = "Installer", module = "portablemc.standard", frozen, subclass)]
pub struct PyInstaller(pub Arc<Mutex<GenericInstaller>>);

#[pymethods]
impl PyInstaller {

    #[new]
    fn __new__(version: &str) -> Self {
        
        let inst = Arc::new(Mutex::new(
            GenericInstaller::Standard(Installer::new(version.to_string()))
        ));

        Self(inst)

    }

    fn __repr__(&self) -> String {
        let guard = self.0.lock().unwrap();
        format!("<portablemc.standard.Installer version={:?}>", guard.standard().version())
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

    fn install(&self) -> PyGame {
        let game = self.0.lock().unwrap().standard_mut().install(())
            .unwrap();  // Change this!
        PyGame(game)
    }

}

#[pyclass(name = "Game", module = "portablemc.standard", frozen)]
pub struct PyGame(pub Game);

#[pymethods]
impl PyGame {

    fn command<'py>(this: &Bound<'py, Self>) -> PyResult<Bound<'py, PyAny>> {
        
        let this = this.borrow();
        let game = &this.0;

        let mod_subprocess = PyModule::import(this.py(), intern!(this.py(), "subprocess"))?;
        let ty_popen = mod_subprocess.getattr(intern!(this.py(), "Popen"))?;

        let mod_functools = PyModule::import(this.py(), intern!(this.py(), "functools"))?;
        let func_partial = mod_functools.getattr(intern!(this.py(), "partial"))?;

        let args = PyList::empty(this.py());
        args.append(&game.jvm_file)?;
        for arg in &game.jvm_args {
            args.append(arg)?;
        }
        args.append(&game.main_class)?;
        for arg in &game.game_args {
            args.append(arg)?;
        }

        let kwargs = [("cwd", &game.mc_dir)].into_py_dict(this.py())?;

        func_partial.call((&ty_popen, &args), Some(&kwargs))

    }

}
