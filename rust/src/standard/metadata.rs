//! Metadata installation stage.

use std::fs::File;
use std::path::{Path, PathBuf};

use super::{serde, Context, EventError};


/// This stage loads the hierarchy of version metadata, this acts as a state-machine and 
/// will iteratively resolve version metadata and returns event.
#[derive(Debug)]
pub struct MetadataLoader<'ctx> {
    context: &'ctx Context,
    next: Option<Next>,
}

/// Internal description of the next version to resolve.
#[derive(Debug)]
struct Next {
    id: String,
    file: Option<PathBuf>,
}

impl<'ctx> MetadataLoader<'ctx> {

    /// Create a new metadata installer with the given context for the given version id.
    pub fn new(context: &'ctx Context, id: String) -> Self {
        Self {
            context,
            next: Some(Next {
                id,
                file: None,
            }),
        }
    }

    /// Advance this 
    pub fn advance(&mut self) -> MetadataEvent<'_> {

        let Some(next) = &mut self.next else {
            return MetadataEvent::Done
        };

        let Some(file) = &next.file else {
            let file = next.file.insert(self.context.version_file(&next.id, "json"));
            return MetadataEvent::Loading { 
                id: &next.id, 
                file: &file,
            };
        };

        /// Read version metadata and wrap event error if relevant.
        fn read_metadata(file: &Path) -> Result<serde::VersionMetadata, EventError> {

            let metadata_reader = File::open(&file)
                .map_err(EventError::Io)?;

            serde_path_to_error::deserialize(&mut serde_json::Deserializer::from_reader(metadata_reader))
                .map_err(EventError::Json)

        }

        // Use the wrapper and reset state to "version loading" in case of error to allow
        // fixing the issue.
        let metadata = match read_metadata(&file) {
            Ok(metadata) => metadata,
            Err(error) => {
                return MetadataEvent::LoadingFailed {
                    id: &next.id,
                    file: &file,
                    error,
                }
            }
        };

        // Take next entry to own the id.
        let next = self.next.take().unwrap();

        // We start by changing the current state to load the inherited metadata.
        // If there is no inherited version, we advance to assets state.
        if let Some(next_version_id) = &metadata.inherits_from {

        } else {

        }

    }

}

#[derive(Debug)]
pub enum MetadataEvent<'a> {
    /// A version is being loaded.
    Loading {
        id: &'a str,
        file: &'a Path,
    },
    /// Parsing of the version JSON failed, the step can be retrieve indefinitely and can
    /// be fixed by writing a valid file at the path, if the error underlying error is
    /// recoverable (file not found, syntax error).
    LoadingFailed {
        id: &'a str,
        file: &'a Path,
        error: EventError,
    },
    /// A version has been loaded from its JSON definition, it is possible to modify the
    /// metadata before releasing borrowing and advancing installation.
    Loaded {
        id: String,
        metadata: Box<serde::VersionMetadata>,
    },
    /// There are no more metadata to iteratively load.
    Done,
}
