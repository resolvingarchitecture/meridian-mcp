//! In-memory session state. Nothing here is ever written to disk.

use std::sync::Mutex;

/// Holds the Architecture Passphrase for the life of the process. The agent supplies it
/// (asking the user when it is missing); it is never persisted or logged.
#[derive(Default)]
pub struct Session {
    passphrase: Mutex<Option<String>>,
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

    /// Forget the passphrase, for example after the backend rejects it.
    pub fn clear(&self) {
        *self.passphrase.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}
