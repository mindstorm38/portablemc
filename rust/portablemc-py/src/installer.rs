//! Generic installer data that is shared between all kind of installers, this is used
//! to allow inheritance.

use portablemc::{base, moj, fabric, forge};


/// A generic class that can be shared inside a `Arc<Mutex<T>>` between.
#[derive(Debug)]
pub enum GenericInstaller {
    Base(base::Installer),
    Mojang(moj::Installer),
    Fabric(fabric::Installer),
    Forge(forge::Installer),
}

impl GenericInstaller {

    pub fn base(&self) -> &base::Installer {
        match self {
            GenericInstaller::Base(installer) => installer,
            GenericInstaller::Mojang(installer) => installer.base(),
            GenericInstaller::Fabric(installer) => installer.mojang().base(),
            GenericInstaller::Forge(installer) => installer.mojang().base(),
        }
    }

    pub fn base_mut(&mut self) -> &mut base::Installer {
        match self {
            GenericInstaller::Base(installer) => installer,
            GenericInstaller::Mojang(installer) => installer.base_mut(),
            GenericInstaller::Fabric(installer) => installer.mojang_mut().base_mut(),
            GenericInstaller::Forge(installer) => installer.mojang_mut().base_mut(),
        }
    }

    pub fn mojang(&self) -> &moj::Installer {
        match self {
            GenericInstaller::Base(_) => panic!("not a mojang installer"),
            GenericInstaller::Mojang(installer) => installer,
            GenericInstaller::Fabric(installer) => installer.mojang(),
            GenericInstaller::Forge(installer) => installer.mojang(),
        }
    }

    pub fn mojang_mut(&mut self) -> &mut moj::Installer {
        match self {
            GenericInstaller::Base(_) => panic!("not a mojang installer"),
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
