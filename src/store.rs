//! Local credential store: what the client must not lose between sessions (an API key, and the claim secret of a
//! pay-first sign-up that is waiting for its payment). It never holds model content, and never the Architecture
//! Passphrase, which stays in memory only (`session.rs`).
//!
//! Where: `MERIDIAN_MCP_HOME`, else the user's config directory (`$XDG_CONFIG_HOME` or `~/.config` on Linux,
//! `~/Library/Application Support` on macOS, `%APPDATA%` on Windows), in a `meridian-mcp` folder.
//!
//! How: a file only the user can read (mode 0600 in a 0700 folder on Unix), written atomically, one entry per backend
//! URL. If `MERIDIAN_VAULT_PASSWORD` is set in the environment, the file is encrypted with it (Argon2id and
//! XChaCha20-Poly1305) and a plaintext file is converted on the next write. The password is read from the environment,
//! never from a tool parameter, so it does not pass through the agent's context. Nothing here is ever logged.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};

const PLAIN_FILE: &str = "credentials.json";
const ENCRYPTED_FILE: &str = "credentials.enc";
const MAGIC: &[u8] = b"MMCP1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;

/// A pay-first sign-up whose payment has been requested. The claim secret is the only way back into it.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PendingSignup {
    pub id: String,
    pub claim_secret: String,
    pub address: Option<String>,
    pub created_at: u64,
}

/// What is kept for one backend.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Account {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signup: Option<PendingSignup>,
}

#[derive(Default, Serialize, Deserialize)]
struct Contents {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    accounts: BTreeMap<String, Account>,
}

/// Where a save went, for the tool result.
pub struct Saved {
    pub path: PathBuf,
    pub encrypted: bool,
}

pub struct Store {
    dir: Option<PathBuf>,
    vault_password: Option<String>,
    backend_url: String,
    write_lock: Mutex<()>,
}

impl Store {
    pub fn open(backend_url: &str) -> Self {
        let vault_password = std::env::var("MERIDIAN_VAULT_PASSWORD").ok().filter(|p| !p.is_empty());
        Self::with(config_dir(), vault_password, backend_url)
    }

    fn with(dir: Option<PathBuf>, vault_password: Option<String>, backend_url: &str) -> Self {
        Self {
            dir,
            vault_password,
            backend_url: backend_url.trim_end_matches('/').to_string(),
            write_lock: Mutex::new(()),
        }
    }

    /// What is stored for this backend (nothing, if there is no file yet).
    pub fn load(&self) -> Result<Account, String> {
        Ok(self.read()?.accounts.remove(&self.backend_url).unwrap_or_default())
    }

    pub fn save_key(&self, api_key: &str) -> Result<Saved, String> {
        self.update(|account| account.api_key = Some(api_key.to_string()))
    }

    pub fn save_signup(&self, signup: PendingSignup) -> Result<Saved, String> {
        self.update(|account| account.signup = Some(signup))
    }

    fn update(&self, change: impl FnOnce(&mut Account)) -> Result<Saved, String> {
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut contents = self.read()?;
        change(contents.accounts.entry(self.backend_url.clone()).or_default());
        contents.version = 1;
        self.write(&contents)
    }

    fn dir(&self) -> Result<&Path, String> {
        self.dir.as_deref().ok_or_else(|| {
            "No config directory found for the credential store. Set MERIDIAN_MCP_HOME to a folder to use.".to_string()
        })
    }

    fn read(&self) -> Result<Contents, String> {
        let dir = self.dir()?;
        let encrypted = dir.join(ENCRYPTED_FILE);
        if encrypted.exists() {
            let Some(password) = &self.vault_password else {
                return Err(format!(
                    "The stored Meridian credentials in {} are encrypted. Set MERIDIAN_VAULT_PASSWORD in this server's \
                     environment to unlock them.",
                    encrypted.display()
                ));
            };
            let bytes = fs::read(&encrypted).map_err(|e| format!("could not read {}: {e}", encrypted.display()))?;
            let plain = decrypt(password, &bytes)?;
            return serde_json::from_slice(&plain).map_err(|_| "The stored credentials could not be read.".to_string());
        }
        let plain = dir.join(PLAIN_FILE);
        match fs::read(&plain) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| format!("The stored credentials in {} could not be read.", plain.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Contents::default()),
            Err(e) => Err(format!("could not read {}: {e}", plain.display())),
        }
    }

    fn write(&self, contents: &Contents) -> Result<Saved, String> {
        let dir = self.dir()?;
        create_private_dir(dir)?;
        let json = serde_json::to_vec(contents).map_err(|e| e.to_string())?;
        match &self.vault_password {
            Some(password) => {
                let path = dir.join(ENCRYPTED_FILE);
                write_atomically(&path, &encrypt(password, &json)?)?;
                // A plaintext copy made before a password was set must not be left behind.
                let _ = fs::remove_file(dir.join(PLAIN_FILE));
                Ok(Saved { path, encrypted: true })
            }
            None => {
                let path = dir.join(PLAIN_FILE);
                write_atomically(&path, &json)?;
                Ok(Saved { path, encrypted: false })
            }
        }
    }
}

pub fn config_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    if let Some(home) = var("MERIDIAN_MCP_HOME") {
        return Some(home);
    }
    if cfg!(windows) {
        return var("APPDATA").map(|d| d.join("meridian-mcp"));
    }
    if let Some(xdg) = var("XDG_CONFIG_HOME") {
        return Some(xdg.join("meridian-mcp"));
    }
    let home = var("HOME")?;
    Some(if cfg!(target_os = "macos") {
        home.join("Library").join("Application Support").join("meridian-mcp")
    } else {
        home.join(".config").join("meridian-mcp")
    })
}

fn create_private_dir(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("could not restrict {}: {e}", dir.display()))?;
    }
    Ok(())
}

/// Writes a temporary file next to the target (readable only by the user), flushes it, then renames it into place, so a
/// crash never leaves a half-written store.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension(format!("tmp{}", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(|e| format!("could not write {}: {e}", path.display()))
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| format!("could not derive the encryption key: {e}"))?;
    Ok(key)
}

fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut bytes = [0u8; N];
    // Two v4 UUIDs are 32 random bytes from the operating system; enough for a salt and a 24-byte nonce.
    let mut pool = Vec::with_capacity(32);
    pool.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    pool.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    bytes.copy_from_slice(&pool[..N]);
    Ok(bytes)
}

fn encrypt(password: &str, plain: &[u8]) -> Result<Vec<u8>, String> {
    let salt = random_bytes::<SALT_LEN>()?;
    let nonce = random_bytes::<NONCE_LEN>()?;
    let key = derive_key(password, &salt)?;
    let cipher = XChaCha20Poly1305::new((&key).into());
    let sealed = cipher
        .encrypt(&XNonce::from(nonce), Payload { msg: plain, aad: MAGIC })
        .map_err(|_| "could not encrypt the credentials".to_string())?;
    let mut out = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + sealed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

fn decrypt(password: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let header = MAGIC.len() + SALT_LEN + NONCE_LEN;
    if bytes.len() <= header || &bytes[..MAGIC.len()] != MAGIC {
        return Err("The stored credentials are not in a format this version understands.".into());
    }
    let salt = &bytes[MAGIC.len()..MAGIC.len() + SALT_LEN];
    let nonce = &bytes[MAGIC.len() + SALT_LEN..header];
    let key = derive_key(password, salt)?;
    let nonce = XNonce::try_from(nonce).map_err(|_| "The stored credentials are damaged.".to_string())?;
    XChaCha20Poly1305::new((&key).into())
        .decrypt(&nonce, Payload { msg: &bytes[header..], aad: MAGIC })
        .map_err(|_| "MERIDIAN_VAULT_PASSWORD does not unlock the stored credentials.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("meridian-mcp-store-{}", uuid::Uuid::new_v4()))
    }

    fn signup() -> PendingSignup {
        PendingSignup { id: "s1".into(), claim_secret: "m_claim_abc".into(), address: Some("bc1qxyz".into()), created_at: 1 }
    }

    #[test]
    fn nothing_stored_is_an_empty_account() {
        let store = Store::with(Some(temp_dir()), None, "https://x/api");
        let account = store.load().unwrap();
        assert!(account.api_key.is_none() && account.signup.is_none());
    }

    #[test]
    fn a_signup_and_a_key_survive_a_restart() {
        let dir = temp_dir();
        let first = Store::with(Some(dir.clone()), None, "https://x/api");
        first.save_signup(signup()).unwrap();
        first.save_key("m_live_key").unwrap();

        let second = Store::with(Some(dir.clone()), None, "https://x/api/");
        let account = second.load().unwrap();
        assert_eq!(account.api_key.as_deref(), Some("m_live_key"));
        assert_eq!(account.signup.unwrap().claim_secret, "m_claim_abc");
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn each_backend_has_its_own_entry() {
        let dir = temp_dir();
        Store::with(Some(dir.clone()), None, "https://a/api").save_key("key-a").unwrap();
        Store::with(Some(dir.clone()), None, "https://b/api").save_key("key-b").unwrap();
        assert_eq!(Store::with(Some(dir.clone()), None, "https://a/api").load().unwrap().api_key.as_deref(), Some("key-a"));
        assert_eq!(Store::with(Some(dir.clone()), None, "https://b/api").load().unwrap().api_key.as_deref(), Some("key-b"));
        fs::remove_dir_all(dir).ok();
    }

    #[cfg(unix)]
    #[test]
    fn the_file_and_folder_are_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir();
        let saved = Store::with(Some(dir.clone()), None, "https://x/api").save_key("k").unwrap();
        assert_eq!(fs::metadata(&saved.path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_vault_password_encrypts_the_file_and_the_key_is_not_readable_in_it() {
        let dir = temp_dir();
        let store = Store::with(Some(dir.clone()), Some("correct horse".into()), "https://x/api");
        let saved = store.save_key("m_live_secret_value").unwrap();
        assert!(saved.encrypted);
        let raw = fs::read(&saved.path).unwrap();
        assert!(!String::from_utf8_lossy(&raw).contains("m_live_secret_value"));
        assert_eq!(store.load().unwrap().api_key.as_deref(), Some("m_live_secret_value"));
        assert!(!dir.join(PLAIN_FILE).exists());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_wrong_or_missing_password_is_refused_and_does_not_overwrite() {
        let dir = temp_dir();
        Store::with(Some(dir.clone()), Some("right".into()), "https://x/api").save_key("k").unwrap();

        let wrong = Store::with(Some(dir.clone()), Some("wrong".into()), "https://x/api");
        assert!(wrong.load().unwrap_err().contains("does not unlock"));
        assert!(wrong.save_key("other").is_err());

        let missing = Store::with(Some(dir.clone()), None, "https://x/api");
        assert!(missing.load().unwrap_err().contains("MERIDIAN_VAULT_PASSWORD"));
        assert!(missing.save_key("other").is_err());

        assert_eq!(Store::with(Some(dir.clone()), Some("right".into()), "https://x/api").load().unwrap().api_key.as_deref(), Some("k"));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn setting_a_password_later_converts_the_plaintext_file() {
        let dir = temp_dir();
        Store::with(Some(dir.clone()), None, "https://x/api").save_key("k1").unwrap();
        let store = Store::with(Some(dir.clone()), Some("pw".into()), "https://x/api");
        assert_eq!(store.load().unwrap().api_key.as_deref(), Some("k1"));
        store.save_signup(signup()).unwrap();
        assert!(!dir.join(PLAIN_FILE).exists() && dir.join(ENCRYPTED_FILE).exists());
        assert_eq!(store.load().unwrap().api_key.as_deref(), Some("k1"));
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn no_config_directory_is_an_error_not_a_panic() {
        let store = Store::with(None, None, "https://x/api");
        assert!(store.save_key("k").is_err());
    }
}
