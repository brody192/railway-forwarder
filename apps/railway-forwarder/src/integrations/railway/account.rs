//! OAuth account metadata and unauthenticated profile-image retrieval.
use super::auth::{self, ISSUER, OAuth};
use crate::domain::{AccountPhoto, clean_name};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use url::Url;

#[derive(Deserialize)]
pub(crate) struct AccountProfile {
    pub email: String,
    pub name: Option<String>,
    pub picture: Option<String>,
}

impl OAuth {
    pub async fn profile(&self) -> Result<AccountProfile> {
        let token = self.access_token().await?;
        let mut profile: AccountProfile = auth::send(
            self.http
                .get(format!("{ISSUER}/oauth/me"))
                .bearer_auth(token),
        )
        .await?
        .json()
        .await
        .context("Could not read the Railway account profile.")?;
        profile.email = profile.email.trim().into();
        if profile.email.is_empty() {
            bail!("Railway returned no account email.");
        }
        profile.name = profile
            .name
            .map(|name| clean_name(&name, ""))
            .filter(|name| !name.is_empty());
        Ok(profile)
    }

    pub async fn profile_photo(&self, location: &str) -> Result<AccountPhoto> {
        let url = Url::parse(location)?;
        if url.scheme() != "https" {
            bail!("The profile photo must use HTTPS.");
        }
        // Photo hosts receive no Railway token. They are separate from the
        // authenticated OAuth user-info request above.
        let mut response = self
            .http
            .get(url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?
            .error_for_status()?;
        let mime_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if !matches!(
            mime_type.as_str(),
            "image/png" | "image/jpeg" | "image/jpg" | "image/webp" | "image/gif"
        ) {
            bail!("The profile photo has an unsupported image format.");
        }
        const LIMIT: usize = 2 * 1024 * 1024;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > LIMIT {
                bail!("The profile photo is too large.");
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(AccountPhoto { mime_type, bytes })
    }
}
