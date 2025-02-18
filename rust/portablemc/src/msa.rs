//! Microsoft Account authentication for Minecraft accounts.

use std::io::{self, BufReader, BufWriter, Read, Seek};
use std::iter::FusedIterator;
use std::time::Duration;
use std::path::{Path, PathBuf};
use std::fmt::Debug;
use std::sync::Arc;
use std::fs::File;

use reqwest::{Client, StatusCode};
use serde_json::json;
use uuid::Uuid;

use jsonwebtoken::{DecodingKey, TokenData, Validation};


/// Microsoft Account authenticator.
/// 
/// See <https://minecraft.wiki/w/Microsoft_authentication>. Shout out to wiki.vg which no 
/// longer exists: <https://wiki.vg/Microsoft_Authentication_Scheme>
#[derive(Debug, Clone)]
pub struct Auth {
    app_id: Arc<str>,
    language_code: Option<String>,
}

impl Auth {

    /// Create a new authenticator with the given application (client) id.
    pub fn new(app_id: &str) -> Self {
        Self {
            app_id: Arc::from(app_id),
            language_code: None,
        }
    }

    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// Define a specific language code to use for localized messages.
    /// 
    /// See <https://en.wikipedia.org/wiki/List_of_ISO_639_language_codes>
    pub fn set_language_code(&mut self, code: impl Into<String>) -> &mut Self {
        self.language_code = Some(code.into());
        self
    }

    /// Request a device code and if successful, returns the device code auth flow that
    /// contains the user code and the verification URI for that, this flow should be
    /// waited in order to get access to a minecraft authenticator that will ultimately
    /// produce the desired username, UUID and its auth token(s).
    /// 
    /// You can opt-in to also request the account's primary email via OpenID MSA scope.
    pub fn request_device_code(&self) -> Result<DeviceCodeFlow, AuthError> {

        crate::tokio::sync(async move {

            // We request the 'XboxLive.signin' and 'offline_access' scopes that are
            // mandatory for the Minecraft authentication.
            // We could also request email with "openid email" scopes.
            let req = MsDeviceAuthRequest {
                client_id: &self.app_id,
                scope: "XboxLive.signin offline_access",
                mkt: self.language_code.as_deref(),
            };

            let client = crate::http::builder().build()?;
            let res = client
                .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
                .form(&req)
                .send().await?
                .error_for_status()?
                .json::<MsDeviceAuthSuccess>().await?;

            Ok(DeviceCodeFlow {
                client,
                app_id: Arc::clone(&self.app_id),
                res,
            })

        })

    }

}

/// Microsoft Account device code flow authenticator.
#[derive(Debug, Clone)]
pub struct DeviceCodeFlow {
    client: Client,
    app_id: Arc<str>,
    res: MsDeviceAuthSuccess,
}

impl DeviceCodeFlow {

    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    pub fn user_code(&self) -> &str {
        &self.res.user_code
    }

    pub fn verification_uri(&self) -> &str {
        &self.res.verification_uri
    }

    pub fn message(&self) -> &str {
        &self.res.message
    }

    /// Wait for the user to authorize via the given user code and verification URI.
    /// If successful the authentication continues and the account is authenticated, if
    /// possible.
    /// 
    /// After a successful answer, this flow object should not be used again!
    pub fn wait(&self) -> Result<Account, AuthError> {

        crate::tokio::sync(async move {

            let req = MsTokenRequest::DeviceCode {
                client_id: &self.app_id,
                device_code: &self.res.device_code,
            };
            
            let interval = Duration::from_secs(self.res.interval as u64);

            loop {

                tokio::time::sleep(interval).await;
                match request_ms_token(&self.client, &req, "XboxLive.signin").await? {
                    Ok(res) => {

                        let mut account = request_minecraft_account(&self.client, &res.access_token).await?;
                        account.app_id = self.app_id.to_string();
                        account.refresh_token = res.refresh_token;

                        break Ok(account);

                    }
                    Err(res) => {
                        match res.error.as_str() {
                            "authorization_pending" => 
                                continue,
                            "authorization_declined" => 
                                break Err(AuthError::AuthorizationDeclined),
                            "expired_token" => 
                                break Err(AuthError::AuthorizationTimedOut),
                            "bad_verification_code" | _ => 
                                break Err(AuthError::Unknown(res.error_description)),
                        }
                    }
                }

            }

        })

    }

}

/// An authenticated and validated Minecraft account.
#[derive(Debug, Clone)]
pub struct Account {
    app_id: String,
    refresh_token: String,
    access_token: String,
    uuid: Uuid,
    username: String,
    xuid: String,
}

impl Account {

    /// The ID of the application that account was authorized for.
    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// The access token to give to Minecraft's AuthLib when starting the game.
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    /// The player's UUID.
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    /// The player's username.
    pub fn username(&self) -> &str {
        &self.username
    }

    /// The Xbox XUID.
    pub fn xuid(&self) -> &str {
        &self.xuid
    }

    /// Make a request of this account's profile, this function take self by mutable 
    /// reference because it may update the username if it has been modified since last
    /// request. If this function returns an error, it may be necessary to refresh the
    /// account.
    /// 
    /// It's not required to run that on newly authenticated or refreshed accounts.
    pub fn request_profile(&mut self) -> Result<(), AuthError> {
        let client = crate::http::builder().build()?;
        let profile = crate::tokio::sync(request_minecraft_profile(&client, &self.access_token))?;
        self.username = profile.name;
        Ok(())
    }

    /// Request a token refresh of this account, this will use the internal refresh token,
    /// this will also update the username, uuid and access token.
    pub fn request_refresh(&mut self) -> Result<(), AuthError> {

        crate::tokio::sync(async move {

            let client = crate::http::builder().build()?;
            let req = MsTokenRequest::RefreshToken { 
                client_id: &self.app_id, 
                scope: Some("XboxLive.signin offline_access"), 
                refresh_token: &self.refresh_token, 
                client_secret: None,
            };
            
            let res = match request_ms_token(&client, &req, "XboxLive.signin").await? {
                Ok(res) => res,
                Err(res) => {
                    return Err(AuthError::Unknown(res.error_description));
                }
            };

            let account = request_minecraft_account(&client, &res.access_token).await?;
            self.refresh_token = res.refresh_token;
            self.access_token = account.access_token;
            self.uuid = account.uuid;
            self.username = account.username;

            Ok(())
            
        })
        
    }

}

/// Request a Minecraft Account token from the given request.
async fn request_ms_token(
    client: &Client,
    req: &MsTokenRequest<'_>,
    expected_scope: &str,
) -> Result<std::result::Result<MsTokenSuccess, MsAuthError>, AuthError> {

    let res = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(req)
        .send().await?;

    match res.status() {
        StatusCode::OK => {
            
            let res = res.json::<MsTokenSuccess>().await?;

            if res.token_type != "Bearer" {
                return Err(AuthError::Unknown(format!("Unexpected token type: {}", res.token_type)));
            } else if res.scope != expected_scope {
                return Err(AuthError::Unknown(format!("Unexpected scope: {}", res.scope)));
            }

            Ok(Ok(res))

        }
        StatusCode::BAD_REQUEST => {
            Ok(Err(res.json::<MsAuthError>().await?))
        }
        status => Err(AuthError::InvalidStatus(status)),
    }
    
}

/// Full procedure to gain access to a real Minecraft account from a given MSA token.
/// The returned account has no client id, no refresh token and no email.
async fn request_minecraft_account(
    client: &Client,
    ms_auth_token: &str,
) -> Result<Account, AuthError> {

    // XBL authentication and authorization...
    let user_res = request_xbl_user(&client, ms_auth_token).await?;
    let xsts_res = request_xbl_xsts(&client, &user_res.token).await?;

    // Now checking coherency...
    if user_res.display_claims.xui.is_empty() 
    || user_res.display_claims.xui != xsts_res.display_claims.xui {
        return Err(AuthError::Unknown(format!("Invalid or incoherent display claims.")))
    }

    let user_hash = xsts_res.display_claims.xui[0].uhs.as_str();
    let xsts_token = xsts_res.token.as_str();

    // Minecraft with XBL...
    let mc_res = request_minecraft_with_xbl(&client, user_hash, xsts_token).await?;
    let mc_res_token = decode_jwt_without_validation::<MinecraftToken>(&mc_res.access_token)?;
    // Minecraft profile...
    let profile_res = request_minecraft_profile(&client, &mc_res.access_token).await?;

    Ok(Account {
        app_id: String::new(),
        refresh_token: String::new(),
        access_token: mc_res.access_token,
        uuid: profile_res.id,
        username: profile_res.name,
        xuid: mc_res_token.claims.xuid,
    })

}

async fn request_xbl_user(
    client: &Client, 
    ms_auth_token: &str,
) -> Result<XblSuccess, AuthError> {

    let req = json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_auth_token}"),
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });

    let res = client
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&req)
        .send().await?;

    match res.status() {
        StatusCode::OK => Ok(res.json::<XblSuccess>().await?),
        status => return Err(AuthError::InvalidStatus(status)),
    }

}

async fn request_xbl_xsts(
    client: &Client, 
    xbl_user_token: &str,
) -> Result<XblSuccess, AuthError> {

    let req = json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_user_token]
        },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    });

    let res = client
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&req)
        .send().await?;

    match res.status() {
        StatusCode::OK => Ok(res.json::<XblSuccess>().await?),
        StatusCode::UNAUTHORIZED => {
            let res = res.json::<XblError>().await?;
            return Err(AuthError::Unknown(res.message));
        }
        status => return Err(AuthError::InvalidStatus(status)),
    }

}

async fn request_minecraft_with_xbl(
    client: &Client, 
    user_hash: &str, 
    xsts_token: &str,
) -> Result<MinecraftWithXblSuccess, AuthError> {

    let req = json!({
        "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}"),
    });

    let res = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&req)
        .send().await?;

    let mc_res = match res.status() {
        StatusCode::OK => res.json::<MinecraftWithXblSuccess>().await?,
        status => return Err(AuthError::InvalidStatus(status)),
    };

    if mc_res.token_type != "Bearer" {
        return Err(AuthError::Unknown(format!("Unexpected token type: {}", mc_res.token_type)));
    }
    
    Ok(mc_res)

}

async fn request_minecraft_profile(
    client: &Client,
    access_token: &str,
) -> Result<MinecraftProfileSuccess, AuthError> {

    let res = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(access_token)
        .send().await?;

    match res.status() {
        StatusCode::OK => Ok(res.json::<MinecraftProfileSuccess>().await?),
        StatusCode::FORBIDDEN => return Err(AuthError::Unknown(format!("Forbidden access to api.minecraftservices.com, likely because the application lacks approval from Mojang, see https://minecraft.wiki/w/Microsoft_authentication."))),
        StatusCode::UNAUTHORIZED => return Err(AuthError::OutdatedToken),
        StatusCode::NOT_FOUND => return Err(AuthError::DoesNotOwnGame),
        status => return Err(AuthError::InvalidStatus(status)),
    }

}

fn decode_jwt_without_validation<T>(token: &str) -> jsonwebtoken::errors::Result<TokenData<T>>
where 
    T: serde::de::DeserializeOwned,
{
    // We don't want to validate the token, just decode its data.
    // See https://github.com/Keats/jsonwebtoken/issues/277.
    let key = DecodingKey::from_secret(&[]);
    let mut validation = Validation::default();
    validation.insecure_disable_signature_validation();
    validation.validate_aud = false;
    jsonwebtoken::decode(token, &key, &validation)
}

/// The error type containing one error for each failed entry in a download batch.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum AuthError {
    /// Reqwest HTTP-related error.
    #[error("reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// A JWT decoding error happened.
    #[error("jwt: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    /// An unknown HTTP status has been received.
    #[error("invalid status: {0}")]
    InvalidStatus(reqwest::StatusCode),
    /// An unknown, unhandled error happened.
    #[error("unknown: {0}")]
    Unknown(String),
    /// Authorization declined by the user.
    #[error("authorization declined")]
    AuthorizationDeclined,
    /// Time out of the authentication flow.
    #[error("authorization timeout")]
    AuthorizationTimedOut,
    #[error("outdated token")]
    OutdatedToken,
    #[error("does not own the game")]
    DoesNotOwnGame,
}

/// (URL encoded)
#[derive(Debug, Clone, serde::Serialize)]
struct MsDeviceAuthRequest<'a> {
    client_id: &'a str,
    scope: &'a str,
    mkt: Option<&'a str>,
}

/// (JSON)
#[derive(Debug, Clone, serde::Deserialize)]
struct MsDeviceAuthSuccess {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[allow(unused)]
    expires_in: u32,
    interval: u32,
    message: String,
}

/// (URL encoded)
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "grant_type")]
enum MsTokenRequest<'a> {
    #[serde(rename = "urn:ietf:params:oauth:grant-type:device_code")]
    DeviceCode {
        client_id: &'a str,
        device_code: &'a str,
    },
    #[serde(rename = "refresh_token")]
    RefreshToken {
        client_id: &'a str,
        scope: Option<&'a str>,
        refresh_token: &'a str,
        client_secret: Option<&'a str>,
    },
}

/// (JSON)
#[derive(Debug, Clone, serde::Deserialize)]
struct MsTokenSuccess {
    /// Always "Bearer"
    token_type: String,
    scope: String,
    #[allow(unused)]
    expires_in: u32,
    access_token: String,
    /// Issued if the original scope parameter included the openid scope
    #[allow(unused)]
    id_token: Option<String>,
    /// Issued if the original scope parameter included offline_access.
    refresh_token: String,
}

/// (JSON) Generic authentication error returned by the API.
#[derive(Debug, Clone, serde::Deserialize)]
struct MsAuthError {
    error: String,
    error_description: String,
    #[allow(unused)]
    trace_id: String,
    #[allow(unused)]
    correlation_id: String,
    #[allow(unused)]
    error_uri: Option<String>,
}

/// (JSON) 
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XblSuccess {
    display_claims: XblDisplayClaims,
    #[allow(unused)]
    issue_instant: String,
    #[allow(unused)]
    not_after: String,
    token: String,
}

/// (JSON)
#[derive(Debug, Clone, serde::Deserialize)]
struct XblDisplayClaims {
    xui: Vec<XblXui>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
struct XblXui {
    uhs: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
#[allow(unused)]
struct XblError {
    identity: String,
    x_err: u32,
    message: String,
    redirect: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct MinecraftWithXblSuccess {
    /// Some UUID, not the account's player UUID.
    #[allow(unused)]
    username: String, 
    /// The actual Minecraft access token to use to launch the game.
    access_token: String,
    token_type: String,
    #[allow(unused)]
    expires_in: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct MinecraftProfileSuccess {
    /// The real UUID of the Minecraft account.
    #[serde(with = "uuid::serde::simple")]
    id: Uuid,
    /// The username of the Minecraft account.
    name: String,
    // skins: Vec<MinecraftProfileSkin>,
    // capes: Vec<MinecraftProfileCape>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(unused)]
struct MinecraftProfileSkin {
    id: Uuid,
    state: String,
    url: String,
    variant: String,
    alias: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(unused)]
struct MinecraftProfileCape {
    id: Uuid,
    state: String,
    url: String,
    alias: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(unused)]
struct OpenIdToken {
    nonce: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct MinecraftToken {
    xuid: String,
}

/// A file-backed database for storing accounts. It allows storing and retrieving 
/// accounts atomically (using shared read and exclusive write property of the underlying
/// filesystem).
#[derive(Debug)]
pub struct Database {
    file: PathBuf,
}

impl Database {

    /// Create a new database at the given location, the parent directory may not exists.
    /// This will not actually load the database contents, but it will 
    pub fn new<P: Into<PathBuf>>(file: P) -> Self {
        Self {
            file: file.into(),
        }
    }

    /// Get the file path.
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Internal function to load the database data.
    fn load(&self) -> Result<Option<DatabaseData>, DatabaseError> {
        
        let reader = match File::open(&self.file) {
            Ok(reader) => reader,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let data = serde_json::from_reader::<_, DatabaseData>(BufReader::new(reader))
            .map_err(|_| DatabaseError::Corrupted)?;

        Ok(Some(data))
        
    }

    /// Internal function to load the database data
    fn load_and_store<F, T>(&self, func: F) -> Result<T, DatabaseError>
    where
        F: for<'a> FnOnce(&'a mut DatabaseData, &'a mut bool) -> T,
    {

        let mut rw = File::options().write(true).read(true).create(true).open(&self.file)?;
        let mut data;

        // If the file is empty, don't try to decode it but create a new empty database!
        if rw.read(&mut [0; 1])? == 0 {
            data = DatabaseData { 
                accounts: Vec::new(),
            };
        } else {

            // Rewind to re-read it from start!
            rw.rewind()?;

            data = serde_json::from_reader::<_, DatabaseData>(BufReader::new(&mut rw))
                .map_err(|_| DatabaseError::Corrupted)?;

        }

        let mut save = false;
        let ret = func(&mut data, &mut save);

        if save {

            rw.rewind()?;
            rw.set_len(0)?;
            
            serde_json::to_writer(BufWriter::new(rw), &data)
                .map_err(|_| DatabaseError::WriteFailed)?;

        }

        Ok(ret)

    }

    /// Load every account in this database and return an iterator over all of them.
    pub fn load_iter(&self) -> Result<DatabaseIter, DatabaseError> {
        self.load().map(|data| {
            DatabaseIter {
                raw: data.map(|data| data.accounts)
                    .unwrap_or_default()
                    .into_iter(),
            }
        })
    }
    
    /// Load an account from its UUID.
    pub fn load_from_uuid(&self, uuid: Uuid) -> Result<Option<Account>, DatabaseError> {
        self.load().map(|data| data.and_then(|data| {
            data.accounts.into_iter()
                .find(|acc| acc.uuid == uuid)
                .map(Account::from)
        }))
    }
    
    /// Load an account from its username, because a username it not guaranteed to be
    /// unique, in case of non-freshed sessions that keep old .
    pub fn load_from_username(&self, username: &str) -> Result<Option<Account>, DatabaseError> {
        self.load().map(|data| data.and_then(|data| {
            data.accounts.into_iter()
                .find(|acc| acc.username == username)
                .map(Account::from)
        }))
    }

    /// Remove the given account from its UUID, if existing, and save the database without
    /// it. 
    /// 
    /// If the account doesn't exist, the database is not touch, only read.
    pub fn remove_from_uuid(&self, uuid: Uuid) -> Result<Option<Account>, DatabaseError> {
        self.load_and_store(|data, save| {
            let index = data.accounts.iter().position(|acc| acc.uuid == uuid)?;
            *save = true;
            Some(data.accounts.remove(index).into())
        })
    }

    /// Remove the given account from its username, if existing, and save the database
    /// without it. Note that a username is not guaranteed to be unique, so only the first
    /// matching account is removed.
    /// 
    /// If the account doesn't exist, the database is not touch, only read.
    pub fn remove_from_username(&self, username: &str) -> Result<Option<Account>, DatabaseError> {
        self.load_and_store(|data, save| {
            let index = data.accounts.iter().position(|acc| acc.username == username)?;
            *save = true;
            Some(data.accounts.remove(index).into())
        })
    }

    /// Store the given account in this database, overwrite any previously stored account
    /// with the same UUID.
    pub fn store(&self, account: Account) -> Result<(), DatabaseError> {
        self.load_and_store(|data, save| {
            *save = true;
            if let Some(index) = data.accounts.iter().position(|acc| acc.uuid == account.uuid) {
                data.accounts[index] = account.into();
            } else {
                data.accounts.push(account.into());
            }
        })
    }

}

/// An iterator over all loader accounts in the database.
pub struct DatabaseIter {
    raw: std::vec::IntoIter<DatabaseDataAccount>,
}

impl FusedIterator for DatabaseIter {  }
impl ExactSizeIterator for DatabaseIter {  }
impl Iterator for DatabaseIter {

    type Item = Account;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.raw.next().map(Account::from)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.raw.size_hint()
    }

}

impl DoubleEndedIterator for DatabaseIter {
    
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.raw.next_back().map(Account::from)
    }

}

/// The error type containing one error for each failed entry in a download batch.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum DatabaseError {
    /// An underlying I/O error.
    #[error("io: {0}")]
    Io(#[from] io::Error),
    /// The database is corrupted and nothing can be done about it automatically, you
    /// can move the file to a backup location before retrying.
    #[error("corrupted")]
    Corrupted,
    #[error("write failed")]
    WriteFailed,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct DatabaseData {
    accounts: Vec<DatabaseDataAccount>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct DatabaseDataAccount {
    app_id: String,
    refresh_token: String,
    access_token: String,
    uuid: Uuid,
    username: String,
    xuid: String,
}

impl From<DatabaseDataAccount> for Account {
    fn from(value: DatabaseDataAccount) -> Self {
        Self {
            app_id: value.app_id,
            refresh_token: value.refresh_token,
            access_token: value.access_token,
            uuid: value.uuid,
            username: value.username,
            xuid: value.xuid,
        }
    }
}

impl From<Account> for DatabaseDataAccount {
    fn from(value: Account) -> Self {
        Self {
            app_id: value.app_id,
            refresh_token: value.refresh_token,
            access_token: value.access_token,
            uuid: value.uuid,
            username: value.username,
            xuid: value.xuid,
        }
    }
}
