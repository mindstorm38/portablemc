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
            GenericInstaller::Fabric(installer) => installer.mojang().standard(),
            GenericInstaller::Forge(installer) => installer.mojang().standard(),
        }
    }

    pub fn standard_mut(&mut self) -> &mut standard::Installer {
        match self {
            GenericInstaller::Standard(installer) => installer,
            GenericInstaller::Mojang(installer) => installer.standard_mut(),
            GenericInstaller::Fabric(installer) => installer.mojang_mut().standard_mut(),
            GenericInstaller::Forge(installer) => installer.mojang_mut().standard_mut(),
        }
    }

    pub fn mojang(&self) -> &mojang::Installer {
        match self {
            GenericInstaller::Standard(_) => panic!("not a mojang installer"),
            GenericInstaller::Mojang(installer) => installer,
            GenericInstaller::Fabric(installer) => installer.mojang(),
            GenericInstaller::Forge(installer) => installer.mojang(),
        }
    }

    pub fn mojang_mut(&mut self) -> &mut mojang::Installer {
        match self {
            GenericInstaller::Standard(_) => panic!("not a mojang installer"),
            GenericInstaller::Mojang(installer) => installer,
            GenericInstaller::Fabric(installer) => installer.mojang_mut(),
            GenericInstaller::Forge(installer) => installer.mojang_mut(),
        }
    }

    pub fn fabric(&self) -> &fabric::Installer {
        match self {
            GenericInstaller::Fabric(installer) => installer,
            _ => panic!("not a fabric installer"),
        }
    }

    pub fn fabric_mut(&mut self) -> &mut fabric::Installer {
        match self {
            GenericInstaller::Fabric(installer) => installer,
            _ => panic!("not a fabric installer"),
        }
    }

    pub fn forge(&self) -> &forge::Installer {
        match self {
            GenericInstaller::Forge(installer) => installer,
            _ => panic!("not a fabric installer"),
        }
    }

    pub fn forge_mut(&mut self) -> &mut forge::Installer {
        match self {
            GenericInstaller::Forge(installer) => installer,
            _ => panic!("not a fabric installer"),
        }
    }
    
}
