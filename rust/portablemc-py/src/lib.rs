//! Python binding for PortableMC.

mod installer;

mod standard;
mod mojang;
mod fabric;
mod forge;

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

    let fabric = PyModule::new(m.py(), "fabric")?;
    fabric::py_module(&fabric)?;
    m.add_submodule(&fabric)?;

    let forge = PyModule::new(m.py(), "forge")?;
    forge::py_module(&forge)?;
    m.add_submodule(&forge)?;
    
    Ok(())

}
