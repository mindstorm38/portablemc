//! Microsoft Account authentication for Minecraft accounts.

use std::time::Duration;
use std::fmt::Debug;

use reqwest::{Client, StatusCode};
use serde_json::json;
use uuid::Uuid;


/// Microsoft Account authenticator.
/// 
/// See https://wiki.vg/Microsoft_Authentication_Scheme
#[derive(Debug, Clone)]
pub struct Auth {
    client_id: String,
    language_code: Option<String>,
}

impl Auth {

    /// Create a new authenticator with the given application (client) id.
    pub fn new(client_id: String) -> Self {
        Self {
            client_id,
            language_code: None,
        }
    }

    /// Define a specific language code to use for localized messages.
    /// 
    /// See https://en.wikipedia.org/wiki/List_of_ISO_639_language_codes
    pub fn with_language_code(mut self, code: String) -> Self {
        self.language_code = Some(code);
        self
    }

    /// Request a device code and if successful, returns the device code auth flow that
    /// contains the user code and the verification URI for that, this flow should be
    /// waited in order to get access to a minecraft authenticator that will ultimately
    /// produce the desired username, UUID and its auth token(s).
    pub fn request_device_code(self) -> Result<DeviceCodeAuth> {

        crate::tokio::sync(async move {

            let req = MsDeviceAuthRequest {
                client_id: &self.client_id,
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

            Ok(DeviceCodeAuth {
                client,
                client_id: self.client_id,
                res,
            })

        })

    }

}

/// Microsoft Account device code flow authenticator.
#[derive(Debug, Clone)]
pub struct DeviceCodeAuth {
    client: Client,
    client_id: String,
    res: MsDeviceAuthSuccess,
}

impl DeviceCodeAuth {

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
    pub fn wait(self) -> Result<Account> {

        crate::tokio::sync(async move {

            let req = MsTokenRequest::DeviceCode {
                client_id: &self.client_id,
                device_code: &self.res.device_code,
            };
            
            let interval = Duration::from_secs(self.res.interval as u64);

            loop {

                tokio::time::sleep(interval).await;
                match request_ms_token(&self.client, &req, "XboxLive.signin").await? {
                    Ok(res) => {

                        let mut account = request_minecraft_account(&self.client, &res.access_token).await?;
                        account.client_id = self.client_id;
                        account.refresh_token = res.refresh_token;

                        break Ok(account);

                    }
                    Err(res) => {
                        match res.error.as_str() {
                            "authorization_pending" => 
                                continue,
                            "authorization_declined" => 
                                break Err(Error::AuthorizationDeclined),
                            "expired_token" => 
                                break Err(Error::AuthorizationTimeOut),
                            "bad_verification_code" | _ => 
                                break Err(Error::UnknownError(res.error_description)),
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
    client_id: String,
    refresh_token: String,
    access_token: String,
    uuid: Uuid,
    username: String,
}

impl Account {

    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn xuid(&self) -> &str {
        ""  // FIXME: TODO:
    }

    /// Make a request of this account's profile, this function take self by mutable 
    /// reference because it may update the username if it has been modified since last
    /// request. If this function returns an error, it may be necessary to refresh the
    /// account.
    /// 
    /// It's not required to run that on newly authenticated or refreshed accounts.
    pub fn request_profile(&mut self) -> Result<()> {
        let client = crate::http::builder().build()?;
        let profile = crate::tokio::sync(request_minecraft_profile(&client, &self.access_token))?;
        self.username = profile.name;
        Ok(())
    }

    /// Request a token refresh of this account, this will use the internal refresh token,
    /// this will also update the username, uuid and access token.
    pub fn request_refresh(&mut self) -> Result<()> {

        crate::tokio::sync(async move {

            let client = crate::http::builder().build()?;
            let req = MsTokenRequest::RefreshToken { 
                client_id: &self.client_id, 
                scope: Some("XboxLive.signin offline_access"), 
                refresh_token: &self.refresh_token, 
                client_secret: None,
            };
            
            let res = match request_ms_token(&client, &req, "XboxLive.signin").await? {
                Ok(res) => res,
                Err(res) => {
                    return Err(Error::UnknownError(res.error_description));
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
) -> Result<std::result::Result<MsTokenSuccess, MsAuthError>> {

    let res = client
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(req)
        .send().await?;

    match res.status() {
        StatusCode::OK => {
            
            let res = res.json::<MsTokenSuccess>().await?;

            if res.token_type != "Bearer" {
                return Err(Error::UnknownError(format!("Unexpected token type: {}", res.token_type)));
            } else if res.scope != expected_scope {
                return Err(Error::UnknownError(format!("Unexpected scope: {}", res.scope)));
            }

            Ok(Ok(res))

        }
        StatusCode::BAD_REQUEST => {
            Ok(Err(res.json::<MsAuthError>().await?))
        }
        status => Err(Error::UnknownStatus(status.as_u16())),
    }
    
}

/// Full procedure to gain access to a real Minecraft account from a given MSA token.
/// The returned account has no client id nor refresh token.
async fn request_minecraft_account(
    client: &Client,
    ms_auth_token: &str,
) -> Result<Account> {

    // XBL authentication and authorization...
    let user_res = request_xbl_user(&client, ms_auth_token).await?;
    let xsts_res = request_xbl_xsts(&client, &user_res.token).await?;

    // Now checking coherency...
    if user_res.display_claims.xui.is_empty() 
    || user_res.display_claims.xui != xsts_res.display_claims.xui {
        return Err(Error::UnknownError(format!("Invalid or incoherent display claims.")))
    }

    let user_hash = xsts_res.display_claims.xui[0].uhs.as_str();
    let xsts_token = xsts_res.token.as_str();

    // Minecraft with XBL...
    let mc_res = request_minecraft_with_xbl(&client, user_hash, xsts_token).await?;

    // Minecraft profile...
    let profile_res = request_minecraft_profile(&client, &mc_res.access_token).await?;

    Ok(Account {
        client_id: String::new(),
        refresh_token: String::new(),
        access_token: mc_res.access_token,
        uuid: profile_res.id,
        username: profile_res.name,
    })

}

async fn request_xbl_user(
    client: &Client, 
    ms_auth_token: &str,
) -> Result<XblSuccess> {

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
        status => return Err(Error::UnknownStatus(status.as_u16())),
    }

}

async fn request_xbl_xsts(
    client: &Client, 
    xbl_user_token: &str,
) -> Result<XblSuccess> {

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
            return Err(Error::UnknownError(res.message));
        }
        status => return Err(Error::UnknownStatus(status.as_u16())),
    }

}

async fn request_minecraft_with_xbl(
    client: &Client, 
    user_hash: &str, 
    xsts_token: &str,
) -> Result<MinecraftWithXblSuccess> {

    let req = json!({
        "identityToken": format!("XBL3.0 x={user_hash};{xsts_token}"),
    });

    let res = client
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&req)
        .send().await?;
    
    let mc_res = match res.status() {
        StatusCode::OK => res.json::<MinecraftWithXblSuccess>().await?,
        status => return Err(Error::UnknownStatus(status.as_u16())),
    };

    if mc_res.token_type != "Bearer" {
        return Err(Error::UnknownError(format!("Unexpected token type: {}", mc_res.token_type)));
    }
    
    Ok(mc_res)

}

async fn request_minecraft_profile(
    client: &Client,
    access_token: &str,
) -> Result<MinecraftProfileSuccess> {

    let res = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(access_token)
        .send().await?;

    match res.status() {
        StatusCode::OK => Ok(res.json::<MinecraftProfileSuccess>().await?),
        StatusCode::FORBIDDEN => return Err(Error::UnknownError(format!("Forbidden access to api.minecraftservices.com, likely because the application lacks approval from Mojang, see https://wiki.vg/Microsoft_Authentication_Scheme."))),
        StatusCode::UNAUTHORIZED => return Err(Error::OutdatedToken),
        StatusCode::NOT_FOUND => return Err(Error::DoesNotOwnGame),
        status => return Err(Error::UnknownStatus(status.as_u16())),
    }

}

/// The error type containing one error for each failed entry in a download batch.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Reqwest HTTP-related error.
    #[error("reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// An unknown HTTP status has been received.
    #[error("unknown status: {0}")]
    UnknownStatus(u16),
    /// An unknown, unhandled error happened.
    #[error("unknown error: {0}")]
    UnknownError(String),
    /// Authorization declined by the user.
    #[error("authorization declined")]
    AuthorizationDeclined,
    /// Time out of the authentication flow.
    #[error("authorization timeout")]
    AuthorizationTimeOut,
    #[error("outdated token")]
    OutdatedToken,
    #[error("does not own the game")]
    DoesNotOwnGame,
}

/// Type alias for a result of batch download.
pub type Result<T> = std::result::Result<T, Error>;

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
    error_uri: String,
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
