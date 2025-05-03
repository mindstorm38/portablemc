use std::sync::{Arc, Mutex};
use std::fmt::Write as _;
use std::path::PathBuf;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use portablemc::mojang::{Installer, QuickPlay, Version};

use crate::installer::GenericInstaller;
use crate::uuid::PyUuid;
use crate::msa;


/// Define the `_portablemc.mojang` submodule.
pub(super) fn py_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyVersion>()?;
    m.add_class::<PyQuickPlay>()?;
    m.add_class::<PyInstaller>()?;
    Ok(())
}

#[pyclass(name = "Version", module = "portablemc.mojang", eq)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PyVersion {
    Release,
    Snapshot,
}

#[derive(FromPyObject, IntoPyObject)]
pub enum PyVersionUnion {
    Version(PyVersion),
    Name(String),
}

impl From<PyVersionUnion> for Version {
    fn from(value: PyVersionUnion) -> Self {
        match value {
            PyVersionUnion::Version(PyVersion::Release) => Version::Release,
            PyVersionUnion::Version(PyVersion::Snapshot) => Version::Snapshot,
            PyVersionUnion::Name(name) => Version::Name(name),
        }
    }
}

#[pyclass(name = "QuickPlay", module = "portablemc.mojang", eq)]
#[derive(Clone, PartialEq, Eq)]
pub enum PyQuickPlay {
    Path {
        path: PathBuf,
    },
    Singleplayer {
        name: String,
    },
    Multiplayer {
        host: String,
        port: u16,
    },
    Realms {
        id: String,
    },
}

#[pyclass(name = "Installer", module = "portablemc.mojang", frozen, subclass, extends = crate::base::PyInstaller)]
pub struct PyInstaller(pub Arc<Mutex<GenericInstaller>>);

#[pymethods]
impl PyInstaller {

    #[new]
    #[pyo3(signature = (version = PyVersionUnion::Version(PyVersion::Release)))]
    fn __new__(version: PyVersionUnion) -> PyClassInitializer<Self> {

        let inst = Arc::new(Mutex::new(
            GenericInstaller::Mojang(Installer::new(version))
        ));
        
        PyClassInitializer::from(crate::base::PyInstaller(Arc::clone(&inst)))
            .add_subclass(Self(inst))

    }

    fn __repr__(&self) -> String {

        let guard = self.0.lock().unwrap();
        let inst = guard.mojang();
        let mut buf = format!("<portablemc.mojang.Installer");
        
        match inst.version() {
            Version::Release => write!(buf, " version=Version.Release").unwrap(),
            Version::Snapshot => write!(buf, " version=Version.Snapshot").unwrap(),
            Version::Name(name) => write!(buf, " version={name:?}").unwrap(),
        }

        write!(buf, ">").unwrap();
        buf
        
    }

    #[getter]
    fn version(&self) -> PyVersionUnion {
        match self.0.lock().unwrap().mojang().version() {
            Version::Release => PyVersionUnion::Version(PyVersion::Release),
            Version::Snapshot => PyVersionUnion::Version(PyVersion::Snapshot),
            Version::Name(name) => PyVersionUnion::Name(name.clone()),
        }
    }

    #[setter]
    fn set_version(&self, version: PyVersionUnion) {
        self.0.lock().unwrap().mojang_mut().set_version(version);
    }

    // TODO: fetch exclude

    #[getter]
    fn demo(&self) -> bool {
        self.0.lock().unwrap().mojang().demo()
    }

    #[setter]
    fn set_demo(&self, demo: bool) {
        self.0.lock().unwrap().mojang_mut().set_demo(demo);
    }

    #[getter]
    fn quick_play(&self) -> Option<PyQuickPlay> {
        self.0.lock().unwrap().mojang().quick_play().map(|m| match m {
            QuickPlay::Path { path } => PyQuickPlay::Path { path: path.clone() },
            QuickPlay::Singleplayer { name } => PyQuickPlay::Singleplayer { name: name.clone() },
            QuickPlay::Multiplayer { host, port } => PyQuickPlay::Multiplayer { host: host.clone(), port: *port },
            QuickPlay::Realms { id } => PyQuickPlay::Realms { id: id.clone() },
        })
    }

    #[setter]
    fn set_quick_play(&self, quick_play: Option<PyQuickPlay>) {
        let mut guard = self.0.lock().unwrap();
        match quick_play {
            None => {
                guard.mojang_mut().remove_quick_play();
            }
            Some(quick_play) => {
                guard.mojang_mut().set_quick_play(match quick_play {
                    PyQuickPlay::Path { path } => QuickPlay::Path { path },
                    PyQuickPlay::Singleplayer { name } => QuickPlay::Singleplayer { name },
                    PyQuickPlay::Multiplayer { host, port } => QuickPlay::Multiplayer { host, port },
                    PyQuickPlay::Realms { id } => QuickPlay::Realms { id },
                });
            }
        }
    }

    #[getter]
    fn resolution(&self) -> Option<(u16, u16)> {
        self.0.lock().unwrap().mojang().resolution()
    }

    #[setter]
    fn set_resolution(&self, resolution: Option<(u16, u16)>) {
        let mut guard = self.0.lock().unwrap();
        match resolution {
            Some((width, height)) => {
                guard.mojang_mut().set_resolution(width, height);
            }
            None => {
                guard.mojang_mut().remove_resolution();
            }
        }
    }

    #[getter]
    fn disable_multiplayer(&self) -> bool {
        self.0.lock().unwrap().mojang().disable_multiplayer()
    }

    #[setter]
    fn set_disable_multiplayer(&self, disable_multiplayer: bool) {
        self.0.lock().unwrap().mojang_mut().set_disable_multiplayer(disable_multiplayer);
    }

    #[getter]
    fn disable_chat(&self) -> bool {
        self.0.lock().unwrap().mojang().disable_chat()
    }

    #[setter]
    fn set_disable_chat(&self, disable_chat: bool) {
        self.0.lock().unwrap().mojang_mut().set_disable_chat(disable_chat);
    }

    #[getter]
    fn auth_uuid(&self) -> PyUuid {
        self.0.lock().unwrap().mojang().auth_uuid().into()
    }

    #[getter]
    fn auth_username(&self) -> String {
        self.0.lock().unwrap().mojang().auth_username().to_string()
    }

    fn set_auth_offline(&self, uuid: PyUuid, username: String) {
        self.0.lock().unwrap().mojang_mut().set_auth_offline(uuid.into(), username);
    }

    fn set_auth_offline_uuid(&self, uuid: PyUuid) {
        self.0.lock().unwrap().mojang_mut().set_auth_offline_uuid(uuid.into());
    }

    fn set_auth_offline_username(&self, username: String) {
        self.0.lock().unwrap().mojang_mut().set_auth_offline_username(username);
    }

    fn set_auth_offline_hostname(&self) {
        self.0.lock().unwrap().mojang_mut().set_auth_offline_hostname();
    }

    fn set_auth_msa(&self, account: PyRef<'_, msa::PyAccount>) {
        self.0.lock().unwrap().mojang_mut().set_auth_msa(&account.0);
    }
    
    #[getter]
    fn client_id(&self) -> String {
        self.0.lock().unwrap().mojang().client_id().to_string()
    }

    #[setter]
    fn set_client_id(&self, client_id: String) {
        self.0.lock().unwrap().mojang_mut().set_client_id(client_id);
    }

    #[getter]
    fn fix_legacy_quick_play(&self) -> bool {
        self.0.lock().unwrap().mojang().fix_legacy_quick_play()
    }

    #[setter]
    fn set_fix_legacy_quick_play(&self, fix: bool) {
        self.0.lock().unwrap().mojang_mut().set_fix_legacy_quick_play(fix);
    }

    #[getter]
    fn fix_legacy_proxy(&self) -> bool {
        self.0.lock().unwrap().mojang().fix_legacy_proxy()
    }

    #[setter]
    fn set_fix_legacy_proxy(&self, fix: bool) {
        self.0.lock().unwrap().mojang_mut().set_fix_legacy_proxy(fix);
    }

    #[getter]
    fn fix_legacy_merge_sort(&self) -> bool {
        self.0.lock().unwrap().mojang().fix_legacy_merge_sort()
    }

    #[setter]
    fn set_fix_legacy_merge_sort(&self, fix: bool) {
        self.0.lock().unwrap().mojang_mut().set_fix_legacy_merge_sort(fix);
    }

    #[getter]
    fn fix_legacy_resolution(&self) -> bool {
        self.0.lock().unwrap().mojang().fix_legacy_resolution()
    }

    #[setter]
    fn set_fix_legacy_resolution(&self, fix: bool) {
        self.0.lock().unwrap().mojang_mut().set_fix_legacy_resolution(fix);
    }

    #[getter]
    fn fix_broken_authlib(&self) -> bool {
        self.0.lock().unwrap().mojang().fix_broken_authlib()
    }

    #[setter]
    fn set_fix_broken_authlib(&self, fix: bool) {
        self.0.lock().unwrap().mojang_mut().set_fix_broken_authlib(fix);
    }

    #[getter]
    fn fix_lwjgl(&self) -> Option<String> {
        self.0.lock().unwrap().mojang().fix_lwjgl().map(str::to_string)
    }

    #[setter]
    fn set_fix_lwjgl(&self, lwjgl_version: Option<String>) {
        let mut guard = self.0.lock().unwrap();
        match lwjgl_version {
            Some(lwjgl_version) => {
                guard.mojang_mut().set_fix_lwjgl(lwjgl_version);
            }
            None => {
                guard.mojang_mut().remove_fix_lwjgl();
            }
        }
    }

    fn install(&self) -> PyResult<crate::base::PyGame> {
        self.0.lock().unwrap().mojang_mut().install(())
            .map(crate::base::PyGame)
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

}
