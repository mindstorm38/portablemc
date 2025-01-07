//! The authentication database for the CLI.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::io::{self, BufReader};
use std::fs::File;

use file_lock::FileLock;
use uuid::Uuid;

use portablemc::msa;


/// Represent a loaded database, linked to a physical file, where authentication accounts
/// can be saved to be reused later without re-authenticating again.
#[derive(Debug)]
pub struct DatabaseLock {
    lock: FileLock,
    inner: CommonDatabase,
}

impl DatabaseLock {

    /// Open the given data from its file and lock it exclusively, this call is blocking
    /// and will wait for the database to become available. The object that is returned
    /// is in itself a lock, and this object should be dropped to unlock the file!
    /// Usually, the reading and modification to the database should be as fast as 
    /// possible.
    pub fn lock<P: AsRef<Path>>(file: P) -> Result<Self> {

        let mut lock = FileLock::lock(file, true, File::options().read(true).create(true))?;

        let inner = serde_json::from_reader(BufReader::new(&mut lock.file))
            .map_err(|_| Error::DatabaseCorrupted)?;

        Ok(Self {
            lock,
            inner,
        })

    }

    pub fn put_msa(&mut self, email: &str, account: msa::Account) {
        self.inner.msa.accounts.insert(email.to_string(), MsaAccount {
            client_id: todo!(),
            refresh_token: todo!(),
            access_token: todo!(),
            uuid: todo!(),
            username: todo!(),
        })
    }

    pub fn get_msa() -> msa::Account {

    }

}


/// The error type containing one error for each failed entry in a download batch.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Delegate I/O error.
    #[error("io: {0}")]
    Io(#[from] io::Error),
    /// The authentication database is corrupted, a JSON deserialization error happened,
    /// one solution is to move the file to some backup location, and retry locking!
    #[error("database corrupted")]
    DatabaseCorrupted,
}

/// Type alias for a result of batch download.
pub type Result<T> = std::result::Result<T, Error>;


#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct CommonDatabase {
    #[serde(default)]
    msa: MsaDatabase,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
struct MsaDatabase {
    client_id: String,
    accounts: HashMap<String, MsaAccount>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct MsaAccount {
    client_id: String,
    refresh_token: String,
    access_token: String,
    uuid: Uuid,
    username: String,
}
