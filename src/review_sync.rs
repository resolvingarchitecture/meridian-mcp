//! Keeps the repository's `reviews/` folder complete.
//!
//! A review can be run where the connector is not: in the web app, or by another agent session. Its report is kept
//! on the server, encrypted, and never reaches the repository by itself. While the connector runs, a background
//! task asks the server which reviews exist for each project it has worked on, and writes any that are not yet in
//! `reviews/`. A review counts as present when a file under `reviews/` records its `evaluation_id`, wherever the
//! file is and whatever it is called.
//!
//! Nothing here overwrites or deletes a file, and a review that was present or fetched once is remembered, so a
//! report the user deletes does not come back. Fetching needs the Architecture Passphrase, which the connector
//! holds only after the agent supplied it; without one the task waits and never asks.

use reqwest::Method;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Notify;

use crate::backend::{Backend, Reply};
use crate::session::Session;

/// How often the server is asked.
const POLL: Duration = Duration::from_secs(60);
/// A review this recent is left to the tool that ran it, which saves it itself a moment after the server does.
const SETTLE_MS: i64 = 120_000;

/// Whether the background sync is on. `MERIDIAN_REVIEW_SYNC=off` turns it off.
pub fn enabled() -> bool {
    !matches!(
        std::env::var("MERIDIAN_REVIEW_SYNC").map(|v| v.trim().to_lowercase()).as_deref(),
        Ok("off" | "false" | "0" | "no")
    )
}

/// The projects the connector has worked on, and the folder each one's reports belong under.
#[derive(Default)]
pub struct Targets {
    map: Mutex<HashMap<String, PathBuf>>,
    wake: Notify,
}

impl Targets {
    /// Notes that `project` is being worked on in `root`. A root the caller named (`explicit`) replaces the one noted
    /// before; otherwise the first one stays. A new or changed target is synced at once.
    pub fn watch(&self, project: &str, root: PathBuf, explicit: bool) {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let changed = match map.get(project) {
            None => true,
            Some(old) => explicit && *old != root,
        };
        if changed {
            map.insert(project.to_owned(), root);
            drop(map);
            self.wake.notify_one();
        }
    }

    fn snapshot(&self) -> Vec<(String, PathBuf)> {
        self.map.lock().unwrap_or_else(|e| e.into_inner()).iter().map(|(p, r)| (p.clone(), r.clone())).collect()
    }
}

/// The reviews already dealt with for each folder, kept outside the repository so it holds nothing to commit.
pub struct Ledger {
    path: Option<PathBuf>,
    lock: Mutex<()>,
}

impl Ledger {
    pub fn open() -> Self {
        Self::at(crate::store::config_dir().map(|d| d.join("review-sync.json")))
    }

    pub fn at(path: Option<PathBuf>) -> Self {
        Self { path, lock: Mutex::new(()) }
    }

    fn read(&self) -> HashMap<String, Vec<String>> {
        self.path
            .as_ref()
            .and_then(|p| fs::read_to_string(p).ok())
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    fn seen(&self, root: &Path) -> HashSet<String> {
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        self.read().remove(&key(root)).unwrap_or_default().into_iter().collect()
    }

    fn remember(&self, root: &Path, ids: &HashSet<String>) {
        let Some(path) = &self.path else { return };
        let _guard = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = self.read();
        let entry = all.entry(key(root)).or_default();
        let before = entry.len();
        let mut known: HashSet<String> = entry.iter().cloned().collect();
        for id in ids {
            if known.insert(id.clone()) {
                entry.push(id.clone());
            }
        }
        if entry.len() == before {
            return;
        }
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        // A ledger that cannot be written only means a deleted report may be fetched again.
        let _ = serde_json::to_string(&all).map(|text| fs::write(path, text));
    }
}

fn key(root: &Path) -> String {
    fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf()).display().to_string()
}

/// Starts the background task. It asks about every watched project each minute, and straight away when one is added.
pub fn spawn(backend: Arc<Backend>, session: Arc<Session>, targets: Arc<Targets>, ledger: Arc<Ledger>) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(POLL) => {}
                _ = targets.wake.notified() => {}
            }
            for (project, root) in targets.snapshot() {
                sync_project(&backend, &session, &ledger, &project, &root).await;
            }
        }
    });
}

/// Brings one project's `reviews/` up to date. Returns the files it wrote. Any failure ends this round quietly: the
/// next round tries again.
pub async fn sync_project(backend: &Backend, session: &Session, ledger: &Ledger, project: &str, root: &Path) -> Vec<PathBuf> {
    let mut written = Vec::new();
    let Reply::Ok(listed) = backend.call(Method::GET, &format!("/projects/{project}/reviews"), None, None, false, None).await
    else {
        return written;
    };
    let reviews_dir = crate::review_report::resolve_destination(root).dir;
    let present = scan_present(&reviews_dir);
    let mut seen = ledger.seen(root);
    let todo = missing(&listed, &present, &seen, now_ms());
    // What is in the folder now is dealt with, so deleting it later does not bring it back.
    ledger.remember(root, &present);
    if todo.is_empty() {
        return written;
    }
    let Some(passphrase) = session.resolve(None) else { return written };
    for id in todo {
        match backend
            .call(Method::GET, &format!("/projects/{project}/reviews/{id}"), None, Some(&passphrase), false, None)
            .await
        {
            Reply::Ok(review) => {
                if let Ok(path) = write_fetched(root, project, &review) {
                    written.push(path);
                    seen.insert(id);
                }
            }
            Reply::PassphraseRequired | Reply::PassphraseIncorrect => {
                // A wrong passphrase must not be tried again every minute; the next tool call asks the user.
                session.clear();
                break;
            }
            Reply::Failed(_) => {}
        }
    }
    ledger.remember(root, &seen);
    written
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// The evaluation ids of the reviews already under `dir`, found in the front matter of its markdown files.
pub fn scan_present(dir: &Path) -> HashSet<String> {
    let mut ids = HashSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(folder) = stack.pop() {
        let Ok(entries) = fs::read_dir(&folder) else { continue };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "md") {
                if let Some(id) = fs::read_to_string(&path).ok().as_deref().and_then(evaluation_id_of) {
                    ids.insert(id);
                }
            }
        }
    }
    ids
}

/// The `evaluation_id` in a report's front matter.
fn evaluation_id_of(text: &str) -> Option<String> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    for line in lines {
        let line = line.trim();
        if line == "---" {
            return None;
        }
        if let Some(value) = line.strip_prefix("evaluation_id:") {
            let id = value.trim();
            return (!id.is_empty()).then(|| id.to_owned());
        }
    }
    None
}

/// The listed reviews that are neither in the folder nor dealt with before, and old enough to leave alone no longer.
pub fn missing(listed: &Value, present: &HashSet<String>, seen: &HashSet<String>, now_ms: i64) -> Vec<String> {
    let Some(items) = listed.get("reviews").and_then(Value::as_array) else { return Vec::new() };
    items
        .iter()
        .filter_map(|r| {
            let id = r.get("evaluationId").and_then(Value::as_str)?;
            let at = r.get("evaluatedAtMs").and_then(Value::as_i64).unwrap_or(0);
            let settled = now_ms - at >= SETTLE_MS;
            (settled && !present.contains(id) && !seen.contains(id)).then(|| id.to_owned())
        })
        .collect()
}

/// Writes a fetched review where reviews of its kind go: the `reviews/` of the repository `root` is in, inside
/// `full/` or `intermediate/`.
pub fn write_fetched(root: &Path, project: &str, review: &Value) -> Result<PathBuf, String> {
    let found = crate::review_report::resolve_destination(root);
    let dir = match review.get("service").and_then(Value::as_str).and_then(crate::review_report::subfolder) {
        Some(sub) => found.dir.join(sub),
        None => found.dir,
    };
    crate::review_report::save_fetched(&dir, project, review).map(|saved| saved.path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("meridian-sync-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn listed(items: Value) -> Value {
        json!({ "reviews": items })
    }

    #[test]
    fn a_report_is_present_when_any_file_records_its_evaluation_id() {
        let dir = temp("scan");
        fs::create_dir_all(dir.join("full")).unwrap();
        fs::write(dir.join("full/a.md"), "---\nservice: REVIEW_FULL\nevaluation_id: abc\n---\n\nbody\n").unwrap();
        fs::write(dir.join("renamed-by-the-user.md"), "---\nevaluation_id: def\n---\nx").unwrap();
        fs::write(dir.join("notes.md"), "no front matter\nevaluation_id: nope\n").unwrap();
        fs::write(dir.join("other.txt"), "---\nevaluation_id: txt\n---\n").unwrap();

        let ids = scan_present(&dir);

        assert_eq!(ids, HashSet::from(["abc".to_owned(), "def".to_owned()]));
    }

    #[test]
    fn only_reviews_that_are_absent_unseen_and_settled_are_missing() {
        let now = 10_000_000;
        let reviews = listed(json!([
            { "evaluationId": "have", "evaluatedAtMs": 1 },
            { "evaluationId": "deleted-by-user", "evaluatedAtMs": 1 },
            { "evaluationId": "new", "evaluatedAtMs": 1 },
            { "evaluationId": "just-finished", "evaluatedAtMs": now - 1000 },
        ]));
        let present = HashSet::from(["have".to_owned()]);
        let seen = HashSet::from(["deleted-by-user".to_owned()]);

        assert_eq!(missing(&reviews, &present, &seen, now), vec!["new".to_owned()]);
        assert!(missing(&json!({}), &present, &seen, now).is_empty());
    }

    #[test]
    fn a_fetched_report_lands_in_its_folder_and_records_that_the_version_was_not_kept() {
        let root = temp("write");
        fs::create_dir_all(root.join(".git")).unwrap();
        let review = json!({ "service": "REVIEW_FULL", "evaluationId": "e1", "evaluatedAt": "2026-11-27T05:53:16.805Z",
                             "modelVersion": 3, "report": "# Review" });

        let path = write_fetched(&root, "proj", &review).unwrap();

        assert!(path.to_string_lossy().contains("reviews/full/"), "{path:?}");
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("evaluation_id: e1") && text.contains("project_version: unknown"), "{text}");
        assert!(text.contains("version_source: not recorded"), "{text}");
        assert_eq!(scan_present(&root.join("reviews")), HashSet::from(["e1".to_owned()]));
    }

    #[test]
    fn the_ledger_remembers_per_folder_and_survives_a_reopen() {
        let dir = temp("ledger");
        let file = dir.join("review-sync.json");
        let root_a = temp("a");
        let root_b = temp("b");
        Ledger::at(Some(file.clone())).remember(&root_a, &HashSet::from(["x".to_owned()]));

        let reopened = Ledger::at(Some(file));
        assert_eq!(reopened.seen(&root_a), HashSet::from(["x".to_owned()]));
        assert!(reopened.seen(&root_b).is_empty());
    }

    /// A one-thread HTTP server that answers the two review endpoints. Returns its address and the requests it saw.
    fn serve(project: &str, ids: Vec<(&'static str, i64)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        let list_path = format!("/mcp/v1/projects/{project}/reviews");
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = head.lines().next().unwrap_or("").to_owned();
                let path = line.split_whitespace().nth(1).unwrap_or("").to_owned();
                let has_passphrase = head.to_lowercase().contains("x-architecture-passphrase: pw");
                log.lock().unwrap().push(format!("{path} pass={has_passphrase}"));
                let (status, body) = if path == list_path {
                    let items: Vec<Value> = ids.iter().map(|(id, at)| json!({ "evaluationId": id, "evaluatedAtMs": at })).collect();
                    ("200 OK", json!({ "reviews": items }).to_string())
                } else if !has_passphrase {
                    ("401 Unauthorized", json!({ "code": "PASSPHRASE_REQUIRED", "message": "p" }).to_string())
                } else {
                    let id = path.rsplit('/').next().unwrap_or("");
                    ("200 OK", json!({ "service": "REVIEW_FULL", "evaluationId": id, "evaluatedAt": "2026-11-27T05:53:16.805Z",
                                       "modelVersion": 3, "report": format!("# Review {id}") }).to_string())
                };
                let reply = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        (addr, seen)
    }

    fn backend(addr: &str) -> Backend {
        Backend::new(crate::config::Config { api_url: addr.to_owned(), api_key: Some("key".to_owned()) }).unwrap()
    }

    #[tokio::test]
    async fn missing_reviews_are_fetched_with_the_passphrase_and_written_once() {
        let (addr, requests) = serve("p1", vec![("e-old", 1), ("e-have", 1)]);
        let root = temp("e2e");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("reviews/full")).unwrap();
        fs::write(root.join("reviews/full/have.md"), "---\nevaluation_id: e-have\n---\nx").unwrap();
        let ledger = Ledger::at(Some(temp("l").join("l.json")));
        let session = Session::default();
        session.resolve(Some("pw".to_owned()));

        let first = sync_project(&backend(&addr), &session, &ledger, "p1", &root).await;
        let second = sync_project(&backend(&addr), &session, &ledger, "p1", &root).await;

        assert_eq!(first.len(), 1, "{first:?}");
        assert!(fs::read_to_string(&first[0]).unwrap().contains("# Review e-old"));
        assert!(second.is_empty(), "nothing is fetched twice");
        let log = requests.lock().unwrap().join("\n");
        assert!(log.contains("/reviews/e-old pass=true"), "{log}");
        assert!(!log.contains("/reviews/e-have"), "a review already in the folder is not fetched: {log}");
    }

    #[tokio::test]
    async fn without_a_passphrase_nothing_is_fetched_and_a_deleted_report_stays_deleted() {
        let (addr, requests) = serve("p2", vec![("e1", 1)]);
        let root = temp("nopass");
        fs::create_dir_all(root.join(".git")).unwrap();
        let ledger = Ledger::at(Some(temp("l2").join("l.json")));
        let session = Session::default();

        assert!(sync_project(&backend(&addr), &session, &ledger, "p2", &root).await.is_empty());
        assert!(!requests.lock().unwrap().iter().any(|r| r.contains("/reviews/e1")), "no fetch without a passphrase");

        session.resolve(Some("pw".to_owned()));
        let written = sync_project(&backend(&addr), &session, &ledger, "p2", &root).await;
        assert_eq!(written.len(), 1);
        fs::remove_file(&written[0]).unwrap();
        // The user removed it; it must not return.
        assert!(sync_project(&backend(&addr), &session, &ledger, "p2", &root).await.is_empty());
    }

    #[tokio::test]
    async fn a_wrong_passphrase_is_dropped_so_it_is_not_tried_again_every_minute() {
        let (addr, _) = serve("p3", vec![("e1", 1)]);
        let root = temp("wrong");
        fs::create_dir_all(root.join(".git")).unwrap();
        let ledger = Ledger::at(Some(temp("l3").join("l.json")));
        let session = Session::default();
        session.resolve(Some("wrong".to_owned()));

        assert!(sync_project(&backend(&addr), &session, &ledger, "p3", &root).await.is_empty());
        assert!(session.resolve(None).is_none(), "the rejected passphrase is forgotten");
    }
}
