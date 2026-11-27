//! In-memory session state. Nothing here is ever written to disk.

use std::collections::HashMap;
use std::sync::Mutex;

/// Holds the Architecture Passphrase for the life of the process. The agent supplies it
/// (asking the user when it is missing). It is never logged and never written to a file of ours; the operating
/// system's keychain may remember it between runs (`secrets.rs`).
#[derive(Default)]
pub struct Session {
    passphrase: Mutex<Option<String>>,
    /// A pay-first sign-up in progress: (signup ID, claim secret). Memory only.
    signup: Mutex<Option<(String, String)>>,
    /// The passphrase the backend accepted for each project this run. A project has its own passphrase, so the
    /// background sync uses these and never the one the agent last typed, which may belong to another project.
    confirmed: Mutex<HashMap<String, String>>,
}

impl Session {
    /// Remember a passphrase supplied on a tool call, or return the one already held.
    pub fn resolve(&self, supplied: Option<String>) -> Option<String> {
        let mut held = self.passphrase.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(p) = supplied.filter(|p| !p.is_empty()) {
            *held = Some(p);
        }
        held.clone()
    }

    /// Notes that the backend accepted `passphrase` for `project`.
    pub fn confirm(&self, project: &str, passphrase: &str) {
        self.confirmed.lock().unwrap_or_else(|e| e.into_inner()).insert(project.to_owned(), passphrase.to_owned());
    }

    /// The passphrase the backend accepted for `project` during this run.
    pub fn confirmed(&self, project: &str) -> Option<String> {
        self.confirmed.lock().unwrap_or_else(|e| e.into_inner()).get(project).cloned()
    }

    pub fn unconfirm(&self, project: &str) {
        self.confirmed.lock().unwrap_or_else(|e| e.into_inner()).remove(project);
    }

    pub fn set_signup(&self, id: String, secret: String) {
        *self.signup.lock().unwrap_or_else(|e| e.into_inner()) = Some((id, secret));
    }

    pub fn signup(&self) -> Option<(String, String)> {
        self.signup.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn clear_signup(&self) {
        *self.signup.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    /// Forget the passphrase, for example after the backend rejects it.
    pub fn clear(&self) {
        *self.passphrase.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}
