//! Small example that shows how to launch Minecraft with the PortableMC API.

use std::collections::HashSet;

use portablemc::forge::{Installer, };



pub fn main() {

    BABRIC_API.loader_stable().game_stable()

    Installer::new(StandardVersion("custom".to_string()))
        .handler(())
        .install();

}
