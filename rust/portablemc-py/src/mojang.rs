use std::sync::{Arc, Mutex};
use std::path::PathBuf;

// use uuid::Uuid;

use pyo3::prelude::*;

use portablemc::mojang::{Installer, QuickPlay, Version};

use crate::installer::GenericInstaller;


/// Define the `_portablemc.mojang` submodule.
pub(super) fn py_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyVersion>()?;
    m.add_class::<PyQuickPlay>()?;
    m.add_class::<PyInstaller>()?;
    Ok(())
}

#[pyclass(eq, name = "Version", module = "portablemc.mojang")]
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

#[pyclass(eq, name = "QuickPlay", module = "portablemc.mojang")]
#[derive(Clone, PartialEq, Eq)]
enum PyQuickPlay {
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

#[pyclass(name = "Installer", module = "portablemc.mojang", frozen, subclass, extends = crate::standard::PyInstaller)]
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
        match quick_play {
            None => {
                self.0.lock().unwrap().mojang_mut().remove_quick_play();
            }
            Some(quick_play) => {
                self.0.lock().unwrap().mojang_mut().set_quick_play(match quick_play {
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
        match resolution {
            Some((width, height)) => {
                self.0.lock().unwrap().mojang_mut().set_resolution(width, height);
            }
            None => {
                self.0.lock().unwrap().mojang_mut().remove_resolution();
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

    // #[getter]
    // fn auth_uuid(&self) -> Uuid {
    //     self.0.lock().unwrap().mojang().auth_uuid()
    // }

    #[getter]
    fn auth_username(&self) -> String {
        self.0.lock().unwrap().mojang().auth_username().to_string()
    }

    // FIXME: wait for uuid support!

    // fn set_auth_offline(&self, uuid: Uuid, username: String) {
    //     self.0.lock().unwrap().mojang_mut().set_auth_offline(uuid, username);
    // }

    // fn set_auth_offline_uuid(&self, uuid: Uuid) {
    //     self.0.lock().unwrap().mojang_mut().set_auth_offline_uuid(uuid);
    // }

    fn set_auth_offline_username(&self, username: String) {
        self.0.lock().unwrap().mojang_mut().set_auth_offline_username(username);
    }

    fn set_auth_offline_hostname(&self) {
        self.0.lock().unwrap().mojang_mut().set_auth_offline_hostname();
    }

    fn set_auth_msa(&self) {
        // TODO:
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
        match lwjgl_version {
            Some(lwjgl_version) => {
                self.0.lock().unwrap().mojang_mut().set_fix_lwjgl(lwjgl_version);
            }
            None => {
                self.0.lock().unwrap().mojang_mut().remove_fix_lwjgl();
            }
        }
    }

}
