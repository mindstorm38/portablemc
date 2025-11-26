//! Python binding for PortableMC.

use std::path::PathBuf;

use portablemc::standard;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;


#[pyclass]
struct StandardInstaller {
    #[allow(unused)]
    inner: standard::Installer,
}

#[pymethods]
impl StandardInstaller {

    #[new]
    #[pyo3(signature = (version, main_dir = None))]
    fn __new__(version: &str, main_dir: Option<&str>) -> PyResult<Self> {

        let main_dir = match main_dir {
            Some(dir) => PathBuf::from(dir.to_string()),
            None => standard::default_main_dir()
                .ok_or_else(|| PyValueError::new_err("no default main directory on your system"))?,
        };

        Ok(Self {
            inner: standard::Installer::new(version.to_string(), main_dir),
        })

    }

}

#[pymodule]
#[pyo3(name = "portablemc")]
fn entry(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<StandardInstaller>()?;
    Ok(())
}
