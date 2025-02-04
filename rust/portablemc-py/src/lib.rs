//! Python binding for PortableMC.

mod installer;

mod standard;
mod mojang;

use pyo3::prelude::*;


#[pymodule]
#[pyo3(name = "_portablemc")]
fn py_module(m: &Bound<'_, PyModule>) -> PyResult<()> {

    let standard = PyModule::new(m.py(), "standard")?;
    standard::py_module(&standard)?;
    m.add_submodule(&standard)?;

    let mojang = PyModule::new(m.py(), "mojang")?;
    mojang::py_module(&mojang)?;
    m.add_submodule(&mojang)?;
    
    Ok(())

}
