use std::path::{Path, PathBuf};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

use portablemc::msa::{Account, Auth, Database, DatabaseIter, DeviceCodeFlow};

use crate::uuid::PyUuid;


/// Define the `_portablemc.msa` submodule.
pub(super) fn py_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyAuth>()?;
    m.add_class::<PyDeviceCodeFlow>()?;
    m.add_class::<PyAccount>()?;
    m.add_class::<PyDatabase>()?;
    Ok(())
}


#[pyclass(name = "Auth", module = "portablemc.msa", frozen)]
pub struct PyAuth(pub Auth);

#[pymethods]
impl PyAuth {

    #[new]
    fn __new__(app_id: &str) -> Self {
        Self(Auth::new(app_id))
    }

    fn __repr__(&self) -> String {
        format!("<portablemc.msa.Auth app_id={:?}>", self.0.app_id())
    }

    #[getter]
    #[inline]
    fn app_id(&self) -> &str {
        self.0.app_id()
    }

    fn request_device_code(&self) -> PyResult<PyDeviceCodeFlow> {
        self.0.request_device_code()
            .map(PyDeviceCodeFlow)
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

}


#[pyclass(name = "DeviceCodeFlow", module = "portablemc.msa", frozen)]
pub struct PyDeviceCodeFlow(pub DeviceCodeFlow);

#[pymethods]
impl PyDeviceCodeFlow {

    fn __repr__(&self) -> String {
        format!("<portablemc.msa.DeviceCodeFlow app_id={:?} user_code={:?} verification_uri={:?}>", 
            self.0.app_id(), 
            self.0.user_code(),
            self.0.verification_uri())
    }

    #[getter]
    #[inline]
    fn app_id(&self) -> &str {
        self.0.app_id()
    }

    #[getter]
    #[inline]
    fn user_code(&self) -> &str {
        self.0.user_code()
    }

    #[getter]
    #[inline]
    fn verification_uri(&self) -> &str {
        self.0.verification_uri()
    }

    #[getter]
    #[inline]
    fn message(&self) -> &str {
        self.0.message()
    }

    fn wait(&self) -> PyResult<PyAccount> {
        self.0.wait()
            .map(PyAccount)
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

}


#[pyclass(name = "Account", module = "portablemc.msa")]
pub struct PyAccount(pub Account);

#[pymethods]
impl PyAccount {

    fn __repr__(&self) -> String {
        format!("<portablemc.msa.Account app_id={:?} uuid={} username={:?}>", 
            self.0.app_id(), 
            self.0.uuid().braced(),
            self.0.username())
    }

    #[getter]
    #[inline]
    fn app_id(&self) -> &str {
        self.0.app_id()
    }

    #[getter]
    #[inline]
    fn access_token(&self) -> &str {
        self.0.access_token()
    }

    #[getter]
    #[inline]
    fn uuid(&self) -> PyUuid {
        self.0.uuid().into()
    }

    #[getter]
    #[inline]
    fn username(&self) -> &str {
        self.0.username()
    }

    #[getter]
    #[inline]
    fn xuid(&self) -> &str {
        self.0.xuid()
    }

    fn request_profile(&mut self) -> PyResult<()> {
        self.0.request_profile()
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

    fn request_refresh(&mut self) -> PyResult<()> {
        self.0.request_refresh()
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

}


#[pyclass(name = "Database", module = "portablemc.msa", frozen)]
pub struct PyDatabase(pub Database);

#[pymethods]
impl PyDatabase {

    #[new]
    fn __new__(file: PathBuf) -> Self {
        Self(Database::new(file))
    }

    fn __repr__(&self) -> String {
        format!("<portablemc.msa.Database file={:?}>", 
            self.0.file())
    }

    #[getter]
    #[inline]
    fn file(&self) -> &Path {
        self.0.file()
    }

    fn load_iter(&self) -> PyResult<PyDatabaseIter> {
        self.0.load_iter()
            .map(PyDatabaseIter)
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

    fn load_from_uuid(&self, uuid: PyUuid) -> PyResult<Option<PyAccount>> {
        self.0.load_from_uuid(uuid.into())
            .map(|acc| acc.map(PyAccount))
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

    fn load_from_username(&self, username: String) -> PyResult<Option<PyAccount>> {
        self.0.load_from_username(&username)
            .map(|acc| acc.map(PyAccount))
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

    fn remove_from_uuid(&self, uuid: PyUuid) -> PyResult<Option<PyAccount>> {
        self.0.remove_from_uuid(uuid.into())
            .map(|acc| acc.map(PyAccount))
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

    fn remove_from_username(&self, username: String) -> PyResult<Option<PyAccount>> {
        self.0.remove_from_username(&username)
            .map(|acc| acc.map(PyAccount))
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

    fn store(&self, account: PyRef<'_, PyAccount>) -> PyResult<()> {
        self.0.store(account.0.clone())
            .map_err(|e| PyValueError::new_err(format!("{e}")))
    }

}


// Internal class!
#[pyclass(name = "_DatabaseIter", module = "portablemc.msa")]
pub struct PyDatabaseIter(pub DatabaseIter);

#[pymethods]
impl PyDatabaseIter {

    fn __iter__(this: PyRef<'_, Self>) -> PyRef<'_, Self> {
        this
    }

    fn __next__(&mut self) -> Option<PyAccount> {
        self.0.next().map(PyAccount)
    }

}
