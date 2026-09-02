//! Operator commands against a running control plane.

use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedSession {
    pub url: String,
    pub access_token: String,
    pub refresh_token: String,
    pub email: String,
}

pub fn session_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("fps/session.json");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".config/fps/session.json");
    }
    PathBuf::from(".fps-session.json")
}

pub fn load_session() -> Result<SavedSession> {
    let path = session_path();
    let bytes =
        fs::read(&path).with_context(|| format!("no saved session at {}", path.display()))?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn save_session(session: &SavedSession) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(session)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(tmp, path)?;
    Ok(())
}

pub fn delete_session() -> Result<()> {
    let path = session_path();
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(fps_branding::user_agent())
        .timeout(std::time::Duration::from_secs(15))
        .build()?)
}

pub async fn login(url: &str, email: &str, password: &str) -> Result<SavedSession> {
    let url = url.trim_end_matches('/');
    let client = client()?;
    let response = client
        .post(format!("{url}/v1/auth/login"))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await?;
    if !response.status().is_success() {
        bail!(
            "login failed: {}",
            response.text().await.unwrap_or_default()
        );
    }
    let body: serde_json::Value = response.json().await?;
    let session = SavedSession {
        url: url.to_string(),
        access_token: body["access_token"]
            .as_str()
            .context("access_token missing")?
            .to_string(),
        refresh_token: body["refresh_token"].as_str().unwrap_or("").to_string(),
        email: email.to_string(),
    };
    save_session(&session)?;
    Ok(session)
}

pub async fn get_json(path: &str) -> Result<serde_json::Value> {
    let session = load_session()?;
    let client = client()?;
    let response = client
        .get(format!("{}{path}", session.url))
        .bearer_auth(&session.access_token)
        .send()
        .await?;
    if !response.status().is_success() {
        bail!("{}: {}", path, response.text().await.unwrap_or_default());
    }
    Ok(response.json().await?)
}

pub fn prompt_password() -> Result<String> {
    eprint!("Password: ");
    io::stderr().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}
