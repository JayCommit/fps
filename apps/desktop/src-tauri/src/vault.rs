//! Session token vault.
//!
//! Preference: OS credential store via the `keyring` crate
//! (Windows Credential Manager, macOS Keychain, Linux Secret Service).
//!
//! Fallback: `{app_data_dir}/vault/session.token` with Unix mode `0600`
//! (directory `0700`). This Cloud Agent / headless Linux VM usually has no
//! Secret Service, so the file fallback is expected here. Operators on
//! Windows/macOS desktops should get the OS keyring path.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;

const SERVICE: &str = "fps-desktop";
const ACCOUNT: &str = "session";
const FILE_NAME: &str = "session.token";

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("keyring: {0}")]
    Keyring(String),
    #[error("vault I/O: {0}")]
    Io(#[from] io::Error),
    #[error("app data directory is required for the file fallback")]
    MissingAppData,
}

#[derive(Debug, Clone)]
pub struct Vault {
    app_data_dir: PathBuf,
}

impl Vault {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self { app_data_dir }
    }

    pub fn store_session_token(&self, token: &str) -> Result<(), VaultError> {
        match keyring_store(token) {
            Ok(()) => Ok(()),
            Err(err) => {
                eprintln!(
                    "desktop vault: OS keyring unavailable ({err}); writing 0600 file fallback under {}",
                    self.app_data_dir.display()
                );
                self.store_file(token)
            }
        }
    }

    pub fn load_session_token(&self) -> Result<Option<String>, VaultError> {
        match keyring_load() {
            Ok(Some(token)) => Ok(Some(token)),
            Ok(None) => self.load_file(),
            Err(_) => self.load_file(),
        }
    }

    pub fn delete_session_token(&self) -> Result<(), VaultError> {
        let keyring_err = keyring_delete().err();
        let file_err = self.delete_file().err();
        if keyring_err.is_some() && file_err.is_some() {
            if let Some(err) = file_err {
                return Err(err);
            }
        }
        Ok(())
    }

    fn vault_dir(&self) -> PathBuf {
        self.app_data_dir.join("vault")
    }

    fn token_path(&self) -> PathBuf {
        self.vault_dir().join(FILE_NAME)
    }

    fn store_file(&self, token: &str) -> Result<(), VaultError> {
        if self.app_data_dir.as_os_str().is_empty() {
            return Err(VaultError::MissingAppData);
        }
        let dir = self.vault_dir();
        fs::create_dir_all(&dir)?;
        set_owner_secret_dir(&dir)?;
        let path = self.token_path();
        let tmp = path.with_extension("token.tmp");
        {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp)?;
            file.write_all(token.as_bytes())?;
            file.sync_all()?;
        }
        set_owner_secret_file(&tmp)?;
        fs::rename(&tmp, &path)?;
        set_owner_secret_file(&path)?;
        Ok(())
    }

    fn load_file(&self) -> Result<Option<String>, VaultError> {
        let path = self.token_path();
        match fs::read_to_string(&path) {
            Ok(s) if s.is_empty() => Ok(None),
            Ok(s) => Ok(Some(s)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(err.into()),
        }
    }

    fn delete_file(&self) -> Result<(), VaultError> {
        let path = self.token_path();
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }
}

fn keyring_store(token: &str) -> Result<(), VaultError> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(map_keyring)?;
    entry.set_password(token).map_err(map_keyring)
}

fn keyring_load() -> Result<Option<String>, VaultError> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(map_keyring)?;
    match entry.get_password() {
        Ok(s) if s.is_empty() => Ok(None),
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(err) => Err(map_keyring(err)),
    }
}

fn keyring_delete() -> Result<(), VaultError> {
    let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(map_keyring)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(map_keyring(err)),
    }
}

fn map_keyring(err: keyring::Error) -> VaultError {
    VaultError::Keyring(err.to_string())
}

#[cfg(unix)]
fn set_owner_secret_dir(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(unix)]
fn set_owner_secret_file(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_owner_secret_dir(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_secret_file(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn file_fallback_round_trip_and_unix_mode() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("fps-vault-{stamp}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let vault = Vault::new(dir.clone());
        vault.store_file("opaque-session-token").unwrap();
        assert_eq!(
            vault.load_file().unwrap().as_deref(),
            Some("opaque-session-token")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(vault.token_path()).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            let dir_mode = fs::metadata(vault.vault_dir()).unwrap().permissions().mode() & 0o777;
            assert_eq!(dir_mode, 0o700);
        }
        vault.delete_file().unwrap();
        assert!(vault.load_file().unwrap().is_none());
        let _ = fs::remove_dir_all(&dir);
    }
}
