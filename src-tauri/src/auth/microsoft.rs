use serde::Deserialize;
use serde_json::json;

use crate::error::{Error, Result};

pub const CLIENT_ID: &str = "90a06a16-16a9-4fae-ab23-6ec5fdd44978";
const SCOPE: &str = "XboxLive.signin offline_access";

const DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBOX_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCode {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct MsToken {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

pub enum PollOutcome {
    Pending,
    SlowDown,
    Token(MsToken),
}

#[derive(Debug, Clone)]
pub struct McAuth {
    pub uuid: String,
    pub name: String,
    pub access_token: String,
    pub expires_in: i64,
}

#[derive(Deserialize)]
struct AadError {
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_description: String,
}

fn aad_message(text: &str) -> String {
    match serde_json::from_str::<AadError>(text) {
        Ok(e) if !e.error_description.is_empty() => {
            e.error_description.lines().next().unwrap_or("").to_string()
        }
        Ok(e) if !e.error.is_empty() => e.error,
        _ => text.chars().take(300).collect(),
    }
}

pub async fn request_device_code(client: &reqwest::Client) -> Result<DeviceCode> {
    let resp = client
        .post(DEVICE_CODE_URL)
        .form(&[("client_id", CLIENT_ID), ("scope", SCOPE)])
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(Error::other(format!(
            "Microsoft rejected the sign-in request ({status}): {}",
            aad_message(&text)
        )));
    }
    Ok(serde_json::from_str(&text)?)
}

#[derive(Deserialize)]
struct MsTokenResp {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

#[derive(Deserialize)]
struct TokenErrorResp {
    error: String,
}

pub async fn poll_token(client: &reqwest::Client, device_code: &str) -> Result<PollOutcome> {
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", CLIENT_ID),
            ("device_code", device_code),
        ])
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;

    if status.is_success() {
        let token: MsTokenResp = serde_json::from_str(&text)?;
        return Ok(PollOutcome::Token(MsToken {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_in: token.expires_in,
        }));
    }

    let err: TokenErrorResp =
        serde_json::from_str(&text).unwrap_or(TokenErrorResp { error: "unknown".into() });
    match err.error.as_str() {
        "authorization_pending" => Ok(PollOutcome::Pending),
        "slow_down" => Ok(PollOutcome::SlowDown),
        "authorization_declined" => Err(Error::other("Sign-in was declined.")),
        "expired_token" => Err(Error::other("The sign-in request expired. Try again.")),
        other => Err(Error::other(format!("Sign-in failed: {other}"))),
    }
}

pub async fn refresh(client: &reqwest::Client, refresh_token: &str) -> Result<MsToken> {
    let resp = client
        .post(TOKEN_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", CLIENT_ID),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ])
        .send()
        .await?
        .error_for_status()?;
    let token: MsTokenResp = resp.json().await?;
    Ok(MsToken {
        access_token: token.access_token,
        refresh_token: token.refresh_token,
        expires_in: token.expires_in,
    })
}

#[derive(Deserialize)]
struct XboxResp {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Deserialize)]
struct Xui {
    uhs: String,
}

#[derive(Deserialize)]
struct XstsErr {
    #[serde(rename = "XErr")]
    xerr: i64,
}

#[derive(Deserialize)]
struct McLoginResp {
    access_token: String,
    expires_in: i64,
}

#[derive(Deserialize)]
struct ProfileResp {
    id: String,
    name: String,
}

fn xsts_error_message(xerr: i64) -> String {
    match xerr {
        2148916233 => "This Microsoft account has no Xbox profile. Create one first.".into(),
        2148916235 => "Xbox Live is not available in your region.".into(),
        2148916236 | 2148916237 => "Adult verification is required on this account.".into(),
        2148916238 => "This is a child account. Add it to a family group first.".into(),
        other => format!("Xbox sign-in failed (code {other})."),
    }
}

pub async fn authenticate_minecraft(
    client: &reqwest::Client,
    ms_access_token: &str,
) -> Result<McAuth> {
    let xbox: XboxResp = client
        .post(XBOX_URL)
        .json(&json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={ms_access_token}")
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let xsts_resp = client
        .post(XSTS_URL)
        .json(&json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbox.token]
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?;

    if !xsts_resp.status().is_success() {
        let text = xsts_resp.text().await?;
        if let Ok(err) = serde_json::from_str::<XstsErr>(&text) {
            return Err(Error::other(xsts_error_message(err.xerr)));
        }
        return Err(Error::other("Xbox sign-in failed."));
    }

    let xsts: XboxResp = xsts_resp.json().await?;
    let uhs = xsts
        .display_claims
        .xui
        .first()
        .map(|x| x.uhs.clone())
        .ok_or_else(|| Error::other("Missing Xbox user hash."))?;

    let mc: McLoginResp = client
        .post(MC_LOGIN_URL)
        .json(&json!({ "identityToken": format!("XBL3.0 x={uhs};{}", xsts.token) }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let profile_resp = client
        .get(PROFILE_URL)
        .bearer_auth(&mc.access_token)
        .send()
        .await?;
    if profile_resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(Error::other(
            "This account does not own Minecraft: Java Edition.",
        ));
    }
    let profile: ProfileResp = profile_resp.error_for_status()?.json().await?;

    Ok(McAuth {
        uuid: profile.id,
        name: profile.name,
        access_token: mc.access_token,
        expires_in: mc.expires_in,
    })
}
