//! UUID type binding for 'uuid.UUID' in Python.

use pyo3::exceptions::PyValueError;
use pyo3::exceptions::PyTypeError;
use pyo3::types::IntoPyDict;
use pyo3::types::PyBytes;
use pyo3::FromPyObject;
use pyo3::prelude::*;
use pyo3::intern;


use uuid::Uuid;


/// A binding for the standard library `uuid.UUID` Python type.
#[derive(Debug)]
#[repr(transparent)]
pub struct PyUuid(pub Uuid);

impl From<Uuid> for PyUuid {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<PyUuid> for Uuid {
    fn from(value: PyUuid) -> Self {
        value.0
    }
}

impl<'py> FromPyObject<'py> for PyUuid {

    fn extract_bound(ob: &Bound<'py, PyAny>) -> PyResult<Self> {
        
        let mod_uuid = PyModule::import(ob.py(), intern!(ob.py(), "uuid"))?;
        let ty_uuid = mod_uuid.getattr(intern!(ob.py(), "UUID"))?;

        if !ob.is_instance(&ty_uuid)? {
            return Err(PyTypeError::new_err("expected uuid.UUID"));
        }

        let bytes = ob.getattr(intern!(ob.py(), "bytes"))?
            .downcast_into::<PyBytes>()?;

        match Uuid::from_slice(bytes.as_bytes()) {
            Ok(uuid) => Ok(Self(uuid)),
            Err(_err) => Err(PyValueError::new_err("given uuid.UUID has invalid bytes")),
        }

    }

}

impl<'py> IntoPyObject<'py> for PyUuid {

    type Target = PyAny;
    type Output = Bound<'py, PyAny>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {

        let mod_uuid = PyModule::import(py, intern!(py, "uuid"))?;
        let ty_uuid = mod_uuid.getattr(intern!(py, "UUID"))?;

        let bytes = PyBytes::new(py, self.0.as_bytes());
        let kwargs = [("bytes", bytes)].into_py_dict(py)?;

        ty_uuid.call((), Some(&kwargs))

    }

}
