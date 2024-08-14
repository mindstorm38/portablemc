//! Authentication session management.

use std::io::Write;

use uuid::{uuid, Uuid};


/// Abstract authentication session as 
pub trait AuthSession {

    /// Returns the username for this session.
    fn username(&self) -> &str;

    /// Returns the UUID for this session.
    fn uuid(&self) -> &Uuid;

    /// Returns the access token of this auth session.
    fn access_token(&self) -> &str;

    /// Returns the XBox UID for this session, empty for non-MSA sessions.
    /// This is used by modern versions.
    fn xuid(&self) -> &str;

}


/// Offline session, this is a special case useful to simplify the start logic. It can
/// be constructed in different ways, specifying optional username and UUID, randomly 
/// generated when unspecified.
pub struct OfflineAuthSession {
    /// The username of this session, will be truncated to 16 characters anyway.
    pub username: String,
    /// The UUID of this session.
    pub uuid: Uuid,
}

impl OfflineAuthSession {

    /// The UUID namespace used in PMC versions prior to v5 to derive 
    const NAMESPACE_PMC: Uuid = uuid!("8df5a464-38de-11ec-aa66-3fd636ee2ed7");

    /// Create a new default offline session that is only derived from this machine's 
    /// network name (hostname) and the PMC namespace with SHA-1 (UUID v5).
    pub fn new() -> Self {
        Self::with_uuid(Uuid::new_v5(&Self::NAMESPACE_PMC, gethostname::gethostname().as_encoded_bytes()))
    }

    /// Create a new offline session with the given UUID, the username is set to the 
    /// first 8 characters of the rendered UUID.
    pub fn with_uuid(uuid: Uuid) -> Self {
        Self {
            username: {
                let mut buf = uuid.to_string();
                buf.truncate(8);
                buf
            },
            uuid,
        }
    }

    /// Create a new offline session with the given username, the UUID is then derived
    /// from this username using a PMC-specific derivation of the username and the PMC
    /// namespace with SHA-1 (UUID v5).
    pub fn with_username(mut username: String) -> Self {
        username.truncate(16);
        Self {
            uuid: Uuid::new_v5(&Self::NAMESPACE_PMC, username.as_bytes()),
            username,
        }
    }

    /// Create a new offline session with the given username, the UUID is then derived 
    /// from this username using the same derivation used by most Mojang clients (versions
    /// to be defined), this produces a MD5 (v3) UUID with `OfflinePlayer:{username}` as
    /// the hashed string. Note that the username is truncated to 16 characters prior
    /// to hashing.
    pub fn with_username_mojang(mut username: String) -> Self {

        username.truncate(16);

        let mut context = md5::Context::new();
        context.write_fmt(format_args!("OfflinePlayer:{username}")).unwrap();
        
        let uuid = uuid::Builder::from_bytes(context.compute().0)
            .with_variant(uuid::Variant::RFC4122)
            .with_version(uuid::Version::Md5)
            .into_uuid();

        Self {
            username,
            uuid,
        }

    }

}

impl AuthSession for OfflineAuthSession {

    fn username(&self) -> &str {
        &self.username
    }

    fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    fn xuid(&self) -> &str {
        ""
    }

    fn access_token(&self) -> &str {
        ""
    }

}
