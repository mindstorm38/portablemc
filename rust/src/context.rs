//! A Minecraft's installation context.

use std::path::{Path, PathBuf};


/// This structure represents the context of a Minecraft's installation.
#[derive(Debug)]
pub struct Context {
    /// The working directory from where the game is run, the game stores thing like 
    /// saves, resource packs, options and mods if relevant.
    pub work_dir: PathBuf,
    /// The versions directory contains one directory per version, each containing the 
    /// version metadata and potentially the version jar file.
    pub versions_dir: PathBuf,
    /// The assets directory contains the whole assets index.
    pub assets_dir: PathBuf,
    /// The libraries directory contains the various Java libraries required by the game.
    pub libraries_dir: PathBuf,
    /// The JVM directory is specific to PortableMC, and contains the official Java 
    /// versions provided by Microsoft for some common architectures.
    pub jvm_dir: PathBuf,
    /// The binary directory contains temporary directories that are used only during the
    /// game's runtime, modern versions no longer use it but it.
    pub bin_dir: PathBuf,
}

impl Default for Context {

    fn default() -> Self {
        todo!("new with default minecraft dir");
    }

}

impl Context {

    /// Create a basic context with all common directories derived from the given main 
    /// directory. The work directory is also set to the main directory.
    pub fn new(main_dir: impl AsRef<Path>) -> Self {
        
        let main_dir: &Path = main_dir.as_ref();

        Self {
            work_dir: main_dir.to_path_buf(),
            versions_dir: main_dir.join("versions"),
            assets_dir: main_dir.join("assets"),
            libraries_dir: main_dir.join("libraries"),
            jvm_dir: main_dir.join("jvm"),
            bin_dir: main_dir.join("bin"),
        }

    }

    /// Change the work directory to the given one.
    pub fn with_work_dir(&mut self, work_dir: impl Into<PathBuf>) -> &mut Self {
        self.work_dir = work_dir.into();
        self
    }

    /// Get a version directory from its version id.
    pub fn get_version_dir(&self, id: &str) -> PathBuf {
        self.versions_dir.join(id)
    }

}
