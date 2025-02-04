//! Generic installer data that is shared between all kind of installers, this is used
//! to allow inheritance.

use portablemc::{standard, mojang, fabric, forge};


/// A generic class that can be shared inside a `Arc<Mutex<T>>` between.
#[derive(Debug)]
pub(crate) enum GenericInstaller {
    Standard(standard::Installer),
    Mojang(mojang::Installer),
    Fabric(fabric::Installer),
    Forge(forge::Installer),
}

impl GenericInstaller {

    pub fn standard(&self) -> &standard::Installer {
        match self {
            GenericInstaller::Standard(installer) => installer,
            GenericInstaller::Mojang(installer) => installer.standard(),
            GenericInstaller::Fabric(installer) => installer.standard(),
            GenericInstaller::Forge(installer) => installer.standard(),
        }
    }

    pub fn standard_mut(&mut self) -> &standard::Installer {
        match self {
            GenericInstaller::Standard(installer) => installer,
            GenericInstaller::Mojang(installer) => installer.standard_mut(),
            GenericInstaller::Fabric(installer) => installer.standard_mut(),
            GenericInstaller::Forge(installer) => installer.standard_mut(),
        }
    }
    
}
