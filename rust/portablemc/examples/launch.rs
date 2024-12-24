//! Small example that shows how to launch Minecraft with the PortableMC API.

use std::collections::HashSet;



pub fn main() {

    Installer::new(StandardVersion("custom".to_string()))
        .handler(())
        .install();

}



pub struct Installer<V> {
    version: V,
}

impl<V, H> Installer<V, H>
where
    V: Version,
    H: Handler + HandlerSupport<V>,
{

    pub fn new(version: V) -> Self {
        Self {
            version,
        }
    }

    pub fn install<H>(&self, handler: H)
    where
        H: Handler + HandlerSupport<V>,
    {

        
        
    }

}

pub trait Version {

}

pub trait HandlerSupport<V: Version> {  }

pub trait Handler {

    fn filter_features(&mut self, features: &mut HashSet<String>) {
        let _ = (features,);
    }

}

impl<V: Version, H: Handler> HandlerSupport<V> for H {  }

impl Handler for () {  }




pub struct StandardVersion(String);

impl Version for StandardVersion {

}

pub trait StandardHandler {

    

}

impl<H: StandardHandler> HandlerSupport<StandardVersion> for H {  }
