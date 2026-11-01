use crate::persistence::{self, Files};
use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use url::Url;

pub const ISSUER: &str = "https://backboard.railway.com";
pub const CALLBACK: &str = "com.brody.railway-forwarder://oauth/callback";
const SCOPES: &str = "openid email profile offline_access project:viewer ssh_keys";
// Keep the existing session file's Foundation reference-date representation.
const REFERENCE_EPOCH: f64 = 978_307_200.;
fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        - REFERENCE_EPOCH
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tokens {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: f64,
}
#[derive(Serialize, Deserialize)]
struct Registration {
    #[serde(rename = "clientID")]
    client_id: String,
    #[serde(rename = "registrationToken")]
    registration_token: Option<String>,
    #[serde(rename = "registrationURI")]
    registration_uri: Option<String>,
}
struct Session {
    tokens: Option<Tokens>,
    client_id: String,
}
pub struct OAuth {
    pub http: reqwest::Client,
    files: Files,
    session: Mutex<Session>,
}
impl OAuth {
    pub fn new(files: Files, override_id: &str, restore: bool) -> Result<(Self, bool)> {
        let tokens = if restore {
            persistence::read::<Tokens>(&files.credentials.join("tokens.json"))?
        } else {
            None
        };
        let client_id = if !override_id.is_empty() {
            override_id.into()
        } else if restore {
            persistence::read::<Registration>(&files.credentials.join("client.json"))?
                .map(|client| client.client_id)
                .unwrap_or_default()
        } else {
            String::new()
        };
        let signed_in = tokens.is_some();
        Ok((
            Self {
                http: reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .build()?,
                files,
                session: Mutex::new(Session { tokens, client_id }),
            },
            signed_in,
        ))
    }
    pub async fn access_token(&self) -> Result<String> {
        // One mutex covers refresh and persistence, including rotating tokens.
        let mut session = self.session.lock().await;
        let tokens = session
            .tokens
            .as_ref()
            .context("Sign in with Railway to continue.")?;
        if tokens.expires_at - now() > 90. {
            return Ok(tokens.access_token.clone());
        }
        let refresh = tokens
            .refresh_token
            .clone()
            .context("Your Railway session expired. Please sign in again.")?;
        self.exchange(
            &mut session,
            &[("grant_type", "refresh_token"), ("refresh_token", &refresh)],
        )
        .await?;
        Ok(session.tokens.as_ref().unwrap().access_token.clone())
    }
    pub async fn sign_in(&self, override_id: &str) -> Result<()> {
        let mut session = self.session.lock().await;
        if !override_id.trim().is_empty() {
            session.client_id = override_id.trim().into();
        } else if let Some(registration) =
            persistence::read::<Registration>(&self.files.credentials.join("client.json"))?
        {
            session.client_id = registration.client_id;
        } else {
            #[derive(Deserialize)]
            struct Created {
                client_id: String,
                registration_access_token: Option<String>,
                registration_client_uri: Option<String>,
            }
            let created: Created = send(self.http.post(format!("{ISSUER}/oauth/register")).json(&serde_json::json!({"client_name":"Railway Forwarder","application_type":"native","token_endpoint_auth_method":"none","redirect_uris":[CALLBACK],"grant_types":["authorization_code","refresh_token"],"response_types":["code"],"scope":SCOPES}))).await?.json().await?;
            let registration = Registration {
                client_id: created.client_id,
                registration_token: created.registration_access_token,
                registration_uri: created.registration_client_uri,
            };
            persistence::write(&self.files.credentials.join("client.json"), &registration)?;
            session.client_id = registration.client_id;
        }
        let verifier = random();
        let state = random();
        let mut url = Url::parse(&format!("{ISSUER}/oauth/auth"))?;
        url.query_pairs_mut().extend_pairs([
            ("response_type", "code"),
            ("client_id", session.client_id.as_str()),
            ("redirect_uri", CALLBACK),
            ("scope", SCOPES),
            ("state", state.as_str()),
            ("code_challenge", challenge(&verifier).as_str()),
            ("code_challenge_method", "S256"),
            ("prompt", "consent"),
        ]);
        let callback = crate::app::events::authenticate(url.to_string()).await?;
        let code = authorization_code(&callback, &state)?;
        self.exchange(
            &mut session,
            &[
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("redirect_uri", CALLBACK),
                ("code_verifier", &verifier),
            ],
        )
        .await
    }
    async fn exchange(&self, session: &mut Session, fields: &[(&str, &str)]) -> Result<()> {
        #[derive(Deserialize)]
        struct Exchanged {
            access_token: String,
            refresh_token: Option<String>,
            expires_in: f64,
        }
        let mut fields = fields.to_vec();
        fields.push(("client_id", &session.client_id));
        let response: Exchanged = send(
            self.http
                .post(format!("{ISSUER}/oauth/token"))
                .form(&fields),
        )
        .await?
        .json()
        .await?;
        let tokens = Tokens {
            access_token: response.access_token,
            refresh_token: response.refresh_token.or_else(|| {
                session
                    .tokens
                    .as_ref()
                    .and_then(|tokens| tokens.refresh_token.clone())
            }),
            expires_at: now() + response.expires_in,
        };
        persistence::write(&self.files.credentials.join("tokens.json"), &tokens)?;
        session.tokens = Some(tokens);
        Ok(())
    }
    pub async fn sign_out(&self) -> Result<()> {
        let mut session = self.session.lock().await;
        persistence::delete(&self.files.credentials.join("tokens.json"))?;
        session.tokens = None;
        Ok(())
    }
}
fn random() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
fn authorization_code(callback: &str, state: &str) -> Result<String> {
    let url = Url::parse(callback).context("Unexpected OAuth callback.")?;
    if url.scheme() != "com.brody.railway-forwarder"
        || url.host_str() != Some("oauth")
        || url.path() != "/callback"
    {
        bail!("Unexpected OAuth callback.");
    }
    let pairs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
    if pairs.get("state").map(String::as_str) != Some(state) {
        bail!("Railway sign in could not be verified. Please sign in again.");
    }
    if pairs.get("iss").is_some_and(|issuer| issuer != ISSUER) {
        bail!("Unexpected OAuth issuer.");
    }
    if let Some(error) = pairs.get("error") {
        bail!("{}", pairs.get("error_description").unwrap_or(error));
    }
    pairs
        .get("code")
        .cloned()
        .context("Railway did not return an authorization code.")
}
pub async fn send(request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
    let response = request
        .send()
        .await
        .context("Could not connect to Railway.")?;
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    let body = response
        .json::<serde_json::Value>()
        .await
        .unwrap_or_default();
    let detail = body["error_description"]
        .as_str()
        .or_else(|| body["error"].as_str())
        .unwrap_or("Please try again.");
    Err(anyhow!("Railway request failed ({status}). {detail}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_and_callback_validation() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(
            authorization_code(&format!("{CALLBACK}?state=ok&code=code"), "ok").unwrap(),
            "code"
        );
        for callback in [
            format!("{CALLBACK}?state=wrong&code=code"),
            format!("{CALLBACK}?state=ok&code=code&iss=https://wrong.example"),
            "https://wrong.example/callback?state=ok&code=code".into(),
        ] {
            assert!(authorization_code(&callback, "ok").is_err());
        }
    }
    #[test]
    fn existing_session_date_and_rotating_fields_are_preserved() {
        let tokens:Tokens=serde_json::from_value(serde_json::json!({"accessToken":"test","refreshToken":"refresh","expiresAt":800000000.})).unwrap();
        assert_eq!(tokens.expires_at + REFERENCE_EPOCH, 1_778_307_200.);
        assert_eq!(
            serde_json::to_value(tokens).unwrap()["refreshToken"],
            "refresh"
        );
    }
}
