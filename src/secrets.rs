//! Remembers the Architecture Passphrase of each project in the operating system's keychain, so the connector can
//! fetch a project's reviews after a restart without being asked again.
//!
//! The keychain is the operating system's own store (Keychain on macOS, Credential Manager on Windows, the Secret
//! Service on Linux): the passphrase is never written to a file of ours, and the connector holds it in memory only while
//! it runs. Where there is no keychain (a headless Linux machine, say) nothing is remembered and the agent supplies the
//! passphrase each session, as before. `MERIDIAN_REMEMBER_PASSPHRASE=off` turns remembering off.

use std::sync::Arc;

const SERVICE: &str = "meridian-mcp";

/// A place to keep one secret per name.
pub trait Keychain: Send + Sync {
    fn get(&self, name: &str) -> Option<String>;
    fn set(&self, name: &str, secret: &str) -> Result<(), String>;
    fn delete(&self, name: &str);
}

/// The operating system's keychain.
pub struct OsKeychain;

#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
impl Keychain for OsKeychain {
    fn get(&self, name: &str) -> Option<String> {
        keyring::Entry::new(SERVICE, name).ok()?.get_password().ok()
    }

    fn set(&self, name: &str, secret: &str) -> Result<(), String> {
        keyring::Entry::new(SERVICE, name)
            .and_then(|e| e.set_password(secret))
            .map_err(|e| format!("the keychain refused it: {e}"))
    }

    fn delete(&self, name: &str) {
        if let Ok(entry) = keyring::Entry::new(SERVICE, name) {
            let _ = entry.delete_credential();
        }
    }
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
impl Keychain for OsKeychain {
    fn get(&self, _name: &str) -> Option<String> {
        None
    }
    fn set(&self, _name: &str, _secret: &str) -> Result<(), String> {
        Err("this system has no supported keychain".to_owned())
    }
    fn delete(&self, _name: &str) {}
}

/// Whether remembering is on. `MERIDIAN_REMEMBER_PASSPHRASE=off` turns it off.
pub fn enabled() -> bool {
    !matches!(
        std::env::var("MERIDIAN_REMEMBER_PASSPHRASE").map(|v| v.trim().to_lowercase()).as_deref(),
        Ok("off" | "false" | "0" | "no")
    )
}

/// The remembered passphrases, one per project and backend.
#[derive(Clone)]
pub struct Passphrases {
    keychain: Option<Arc<dyn Keychain>>,
    backend_url: String,
}

impl Passphrases {
    pub fn os(backend_url: &str) -> Self {
        Self { keychain: enabled().then(|| Arc::new(OsKeychain) as Arc<dyn Keychain>), backend_url: backend_url.trim_end_matches('/').to_owned() }
    }

    #[cfg(test)]
    pub fn with(keychain: Arc<dyn Keychain>, backend_url: &str) -> Self {
        Self { keychain: Some(keychain), backend_url: backend_url.trim_end_matches('/').to_owned() }
    }

    fn name(&self, project: &str) -> String {
        format!("{}|{project}", self.backend_url)
    }

    /// Keychain calls can block (the Secret Service is a round trip over D-Bus), so they run off the async threads.
    pub async fn get(&self, project: &str) -> Option<String> {
        let keychain = self.keychain.clone()?;
        let name = self.name(project);
        tokio::task::spawn_blocking(move || keychain.get(&name)).await.ok().flatten()
    }

    /// Remembers it, unless it is what is already kept. A keychain that refuses is not an error worth stopping for.
    pub async fn remember(&self, project: &str, passphrase: &str) {
        let Some(keychain) = self.keychain.clone() else { return };
        let (name, secret) = (self.name(project), passphrase.to_owned());
        let _ = tokio::task::spawn_blocking(move || {
            if keychain.get(&name).as_deref() != Some(secret.as_str()) {
                let _ = keychain.set(&name, &secret);
            }
        })
        .await;
    }

    pub async fn forget(&self, project: &str) {
        let Some(keychain) = self.keychain.clone() else { return };
        let name = self.name(project);
        let _ = tokio::task::spawn_blocking(move || keychain.delete(&name)).await;
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// A keychain in memory, for tests.
    #[derive(Default)]
    pub struct MemoryKeychain(pub Mutex<HashMap<String, String>>);

    impl Keychain for MemoryKeychain {
        fn get(&self, name: &str) -> Option<String> {
            self.0.lock().unwrap().get(name).cloned()
        }
        fn set(&self, name: &str, secret: &str) -> Result<(), String> {
            self.0.lock().unwrap().insert(name.to_owned(), secret.to_owned());
            Ok(())
        }
        fn delete(&self, name: &str) {
            self.0.lock().unwrap().remove(name);
        }
    }

    #[tokio::test]
    async fn a_passphrase_is_kept_per_project_and_backend_and_can_be_forgotten() {
        let store = Arc::new(MemoryKeychain::default());
        let here = Passphrases::with(store.clone(), "https://a.example/api/");
        let other = Passphrases::with(store.clone(), "https://b.example/api");

        here.remember("p1", "one").await;
        here.remember("p2", "two").await;

        assert_eq!(here.get("p1").await.as_deref(), Some("one"));
        assert_eq!(here.get("p2").await.as_deref(), Some("two"));
        assert_eq!(other.get("p1").await, None, "another backend has its own");
        here.forget("p1").await;
        assert_eq!(here.get("p1").await, None);
        assert_eq!(here.get("p2").await.as_deref(), Some("two"));
    }

    #[tokio::test]
    async fn without_a_keychain_nothing_is_kept() {
        let none = Passphrases { keychain: None, backend_url: "x".into() };
        none.remember("p", "secret").await;
        assert_eq!(none.get("p").await, None);
    }

    /// Needs a real keychain (a desktop session); run with `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn the_operating_systems_keychain_round_trips_a_passphrase() {
        let store = Passphrases { keychain: Some(Arc::new(OsKeychain)), backend_url: "meridian-mcp-test".into() };
        store.remember("round-trip", "s3cret").await;
        assert_eq!(store.get("round-trip").await.as_deref(), Some("s3cret"));
        store.forget("round-trip").await;
        assert_eq!(store.get("round-trip").await, None);
    }
}
