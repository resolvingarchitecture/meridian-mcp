//! The change an intermediate review looks at, and the source evidence kept current with the code.
//!
//! A change is a unified git diff collected from the local repository, together with the current content of the files it
//! touched. The diff is sent for the one review and not stored; the files go to the project's evidence afterwards
//! (`evidence/sync`), so the evidence always matches the code as of the last reviewed revision.
//!
//! The backend knows nothing about git: it keeps an opaque *revision* label (here, the HEAD commit, with `+dirty` when the
//! tree has uncommitted changes) and per-file hashes (git blob hashes). Everything git-specific is in this file, so another
//! version control system needs only its own collector.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Largest diff sent for one review; the backend refuses more.
pub const MAX_CHANGE_BYTES: usize = 512 * 1024;
/// Largest file kept as evidence; the backend refuses more.
pub const MAX_FILE_BYTES: usize = 256 * 1024;
/// Most content in one sync call, so a call fits the backend's 1 MiB request limit with JSON overhead.
pub const MAX_BATCH_BYTES: usize = 600 * 1024;
/// Most files in one sync call (the backend takes 500).
pub const MAX_BATCH_FILES: usize = 400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceFile {
    pub path: String,
    pub content: String,
    pub content_hash: String,
}

#[derive(Debug)]
pub struct Change {
    /// A unified diff.
    pub diff: String,
    /// What the change is, in a few words: the last commit's subject when the change is a commit.
    pub summary: Option<String>,
    /// Which range the diff covers, for the caller to show.
    pub described_as: String,
    /// What the evidence will reflect once this change is synced: HEAD, with `+dirty.<hash>` when there are uncommitted changes.
    pub revision: String,
    /// The commit the diff runs from, when it runs from one.
    pub base_revision: Option<String>,
    /// The current content of the changed files that can be kept as evidence.
    pub files: Vec<EvidenceFile>,
    /// Paths whose evidence must go: deleted, or too large or not text to keep.
    pub removed: Vec<String>,
    /// Something the caller should know, such as why the usual starting point was not used.
    pub note: Option<String>,
    /// The revision the backend holds is not in this repository (history was rewritten), so the evidence should be reconciled.
    pub base_missing: bool,
}

/// What the backend holds as evidence for the project.
#[derive(Debug, Default)]
pub struct StoredEvidence {
    pub revision: Option<String>,
    /// Path and the hash it was stored under.
    pub files: Vec<(String, Option<String>)>,
}

impl StoredEvidence {
    pub fn from_json(v: &serde_json::Value) -> StoredEvidence {
        StoredEvidence {
            revision: v.get("revision").and_then(|r| r.as_str()).map(str::to_owned),
            files: v
                .get("files")
                .and_then(|f| f.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|f| {
                            Some((
                                f.get("path")?.as_str()?.to_owned(),
                                f.get("contentHash").and_then(|h| h.as_str()).map(str::to_owned),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

fn run(repo: &Path, args: &[&str], ok_codes: &[i32]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    let code = out.status.code().unwrap_or(-1);
    if !out.status.success() && !ok_codes.contains(&code) {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    run(repo, args, &[])
}

/// The top of the repository around `repo`, so paths from git (relative to it) can be read from disk.
fn top(repo: &Path) -> Result<PathBuf, String> {
    git(repo, &["rev-parse", "--show-toplevel"])
        .map(|s| PathBuf::from(s.trim()))
        .map_err(|_| format!("{} is not inside a git repository, so there is no change to review.", repo.display()))
}

/// The label for what the working tree is now: the HEAD commit, plus `+dirty.<hash>` when anything is uncommitted or
/// untracked, the hash covering that content, so two different uncommitted states never share a label.
pub fn revision_label(repo: &Path) -> Result<String, String> {
    let top = top(repo)?;
    let head = git(&top, &["rev-parse", "HEAD"]).map_err(|_| "the repository has no commits yet".to_string())?;
    let head = head.trim();
    if git(&top, &["status", "--porcelain"])?.trim().is_empty() {
        return Ok(head.to_owned());
    }
    let mut state = git(&top, &["diff", "--no-color", "--no-ext-diff", "--no-renames", "HEAD", "--"]).unwrap_or_default();
    let mut untracked = untracked_files(&top)?;
    untracked.sort();
    let hashes = hash_paths(&top, &untracked.iter().filter(|p| !p.contains('\n')).map(String::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    for (path, hash) in untracked.iter().zip(hashes) {
        state.push_str(&format!("untracked {path} {hash}\n"));
    }
    let digest = hash_text(&top, &state)?;
    Ok(format!("{head}+dirty.{}", &digest[..digest.len().min(8)]))
}

fn hash_text(top: &Path, text: &str) -> Result<String, String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(top)
        .args(["hash-object", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run git: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let input = text.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let out = child.wait_with_output().map_err(|e| format!("git hash-object failed: {e}"))?;
    let _ = writer.join();
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// The commit a revision label names (the part before `+dirty`).
fn commit_of(label: &str) -> &str {
    label.split('+').next().unwrap_or(label)
}

fn commit_exists(top: &Path, commit: &str) -> bool {
    !commit.is_empty() && !commit.starts_with('-') && git(top, &["cat-file", "-e", &format!("{commit}^{{commit}}")]).is_ok()
}

/// Splits `git ... --name-status -z` output into (status letter, path) pairs. Renames are turned off, so a rename is a
/// delete and an add.
fn parse_name_status(out: &str) -> Vec<(char, String)> {
    let mut tokens = out.split('\0').filter(|t| !t.is_empty());
    let mut pairs = Vec::new();
    while let (Some(status), Some(path)) = (tokens.next(), tokens.next()) {
        if let Some(letter) = status.chars().next() {
            pairs.push((letter, path.to_owned()));
        }
    }
    pairs
}

/// Collects the change to review.
/// - With `base`: everything since that git ref, committed or not.
/// - Else, with the revision the backend's evidence reflects (`evidence_revision`) when this repository has it: everything
///   since that commit, so no commit is skipped between reviews.
/// - Else the uncommitted changes (staged, unstaged, and new files); when there are none, the last commit.
pub fn collect(repo: &Path, base: Option<&str>, evidence_revision: Option<&str>) -> Result<Change, String> {
    let top = top(repo)?;
    let revision = revision_label(&top)?;
    let mut note = None;
    let mut base_missing = false;

    let wanted: Option<(String, &'static str)> = match base.map(str::trim).filter(|b| !b.is_empty()) {
        Some(b) if b.starts_with('-') => return Err("base must be a git ref (a branch, tag, or commit), not an option.".into()),
        Some(b) => Some((b.to_owned(), "base")),
        None => match evidence_revision.map(commit_of).filter(|c| !c.is_empty()) {
            Some(c) if commit_exists(&top, c) => Some((c.to_owned(), "evidence")),
            Some(c) => {
                base_missing = true;
                note = Some(format!(
                    "The revision the evidence reflects ({}) is not in this repository, so history was rewritten. The evidence was \
                     reconciled with the files as they are now, and this review covers the uncommitted changes or the last commit.",
                    c.chars().take(12).collect::<String>()
                ));
                None
            }
            None => None,
        },
    };

    if let Some((_, "evidence")) = &wanted {
        if evidence_revision == Some(revision.as_str()) {
            return Err(format!("Nothing has changed since the last reviewed revision ({}).", short(&revision)));
        }
    }

    let (mut diff, described_as, summary, base_revision, listing): (String, String, Option<String>, Option<String>, Vec<(char, String)>) =
        match &wanted {
            Some((reference, why)) => {
                let sha = git(&top, &["rev-parse", "--verify", &format!("{reference}^{{commit}}")])
                    .map_err(|e| format!("could not resolve {reference}: {e}"))?
                    .trim()
                    .to_owned();
                let diff = git(&top, &["diff", "--no-color", "--no-ext-diff", "--no-renames", reference, "--"])
                    .map_err(|e| format!("could not diff against {reference}: {e}"))?;
                let listing = parse_name_status(&git(&top, &["diff", "--name-status", "-z", "--no-renames", reference, "--"])?);
                let described = if *why == "base" {
                    format!("all changes since {reference}")
                } else {
                    format!("all changes since the last reviewed revision ({})", short(reference))
                };
                (diff, described, None, Some(sha), listing)
            }
            None => {
                let working = git(&top, &["diff", "--no-color", "--no-ext-diff", "--no-renames", "HEAD", "--"]).unwrap_or_default();
                let untracked = untracked_files(&top)?;
                if !working.trim().is_empty() || !untracked.is_empty() {
                    let listing = parse_name_status(
                        &git(&top, &["diff", "--name-status", "-z", "--no-renames", "HEAD", "--"]).unwrap_or_default(),
                    );
                    (working, "uncommitted changes".to_string(), None, None, listing)
                } else {
                    let last = git(&top, &["show", "--no-color", "--no-ext-diff", "--no-renames", "--format=", "HEAD", "--"])
                        .map_err(|e| format!("there are no uncommitted changes and no commit to review: {e}"))?;
                    let listing = parse_name_status(
                        &git(&top, &["show", "--name-status", "-z", "--no-renames", "--format=", "HEAD", "--"]).unwrap_or_default(),
                    );
                    let subject = git(&top, &["log", "-1", "--format=%s"]).ok().map(|s| s.trim().to_string());
                    let parent = git(&top, &["rev-parse", "--verify", "HEAD~1"]).ok().map(|s| s.trim().to_owned());
                    (last, "the last commit".to_string(), subject.filter(|s| !s.is_empty()), parent, listing)
                }
            }
        };

    // New files git does not track yet are part of the change too.
    let untracked = if described_as.starts_with("the last commit") { Vec::new() } else { untracked_files(&top)? };
    for path in &untracked {
        if let Some(text) = new_file_diff(&top, path) {
            diff.push_str(&text);
        }
    }

    if diff.trim().is_empty() && untracked.is_empty() {
        return Err(format!("There is no change to review ({described_as} is empty)."));
    }
    if diff.len() > MAX_CHANGE_BYTES {
        return Err(format!(
            "The change ({described_as}) is {} KiB; one review takes at most {} KiB. Review a smaller range, for example with a nearer `base`.",
            diff.len() / 1024,
            MAX_CHANGE_BYTES / 1024
        ));
    }

    let mut changed: Vec<String> = Vec::new();
    let mut gone: Vec<String> = Vec::new();
    for (status, path) in listing {
        if status == 'D' {
            gone.push(path);
        } else {
            changed.push(path);
        }
    }
    changed.extend(untracked);
    let (files, unkeepable) = read_files(&top, &changed)?;
    gone.extend(unkeepable);
    let present: HashSet<&String> = files.iter().map(|f| &f.path).collect();
    gone.retain(|p| !present.contains(p));
    gone.sort();
    gone.dedup();

    Ok(Change { diff, summary, described_as, revision, base_revision, files, removed: gone, note, base_missing })
}

fn short(label: &str) -> String {
    commit_of(label).chars().take(10).collect()
}

fn untracked_files(top: &Path) -> Result<Vec<String>, String> {
    Ok(git(top, &["ls-files", "--others", "--exclude-standard", "-z"])?
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_owned)
        .collect())
}

/// A unified diff that adds `path` in full, for a file git does not track yet. Binary and oversized files give nothing.
fn new_file_diff(top: &Path, path: &str) -> Option<String> {
    let meta = std::fs::symlink_metadata(top.join(path)).ok()?;
    if !meta.is_file() || meta.len() as usize > MAX_FILE_BYTES {
        return None;
    }
    // `--no-index` exits 1 when the files differ, which they always do here.
    let out = run(top, &["diff", "--no-color", "--no-ext-diff", "--no-index", "--", "/dev/null", path], &[1]).ok()?;
    if out.contains("Binary files") { None } else { Some(out) }
}

/// Reads each path's current content from disk. Returns the files that can be kept, and the paths that cannot (gone,
/// not a regular file, too large, or not text), whose evidence should be dropped rather than left describing old code.
fn read_files(top: &Path, paths: &[String]) -> Result<(Vec<EvidenceFile>, Vec<String>), String> {
    let mut texts: Vec<(String, String)> = Vec::new();
    let mut unkeepable = Vec::new();
    for path in paths {
        match read_text(top, path) {
            Some(text) => texts.push((path.clone(), text)),
            None => unkeepable.push(path.clone()),
        }
    }
    let hashes = hash_paths(top, &texts.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>())?;
    let files = texts
        .into_iter()
        .zip(hashes)
        .map(|((path, content), content_hash)| EvidenceFile { path, content, content_hash })
        .collect();
    Ok((files, unkeepable))
}

fn read_text(top: &Path, path: &str) -> Option<String> {
    let full = top.join(path);
    let meta = std::fs::symlink_metadata(&full).ok()?;
    if !meta.is_file() || meta.len() as usize > MAX_FILE_BYTES {
        return None;
    }
    let bytes = std::fs::read(&full).ok()?;
    if bytes.contains(&0) {
        return None;
    }
    let text = String::from_utf8(bytes).ok()?;
    if text.trim().is_empty() { None } else { Some(text) }
}

/// Git's own hash of each file's current content (one process for all), the same value `git` would give the blob.
fn hash_paths(top: &Path, paths: &[&str]) -> Result<Vec<String>, String> {
    if paths.is_empty() {
        return Ok(Vec::new());
    }
    if paths.iter().any(|p| p.contains('\n')) {
        return Err("a file name contains a newline".into());
    }
    let mut child = Command::new("git")
        .arg("-C")
        .arg(top)
        .args(["hash-object", "--stdin-paths"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run git: {e}"))?;
    let input = paths.join("\n") + "\n";
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    // Write on another thread: git answers as it reads, and a long list could otherwise fill both pipes.
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let out = child.wait_with_output().map_err(|e| format!("git hash-object failed: {e}"))?;
    let _ = writer.join();
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let hashes: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_owned).collect();
    if hashes.len() != paths.len() {
        return Err("git hash-object returned the wrong number of hashes".into());
    }
    Ok(hashes)
}

/// What to send so the backend's evidence matches the working tree: for each file the backend already holds, the files whose
/// content differs and the paths that are gone. It never adds files the backend does not hold; which files count as
/// evidence is the agent's choice. Used before a full review, and after history was rewritten.
pub struct Reconcile {
    pub files: Vec<EvidenceFile>,
    pub removed: Vec<String>,
    pub revision: String,
}

pub fn reconcile(repo: &Path, stored: &StoredEvidence) -> Result<Reconcile, String> {
    let top = top(repo)?;
    let revision = revision_label(&top)?;
    let mut present: Vec<(String, String)> = Vec::new();
    let mut removed = Vec::new();
    for (path, _) in &stored.files {
        match read_text(&top, path) {
            Some(text) => present.push((path.clone(), text)),
            None => removed.push(path.clone()),
        }
    }
    let hashes = hash_paths(&top, &present.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>())?;
    let stored_hash: std::collections::HashMap<&String, &Option<String>> = stored.files.iter().map(|(p, h)| (p, h)).collect();
    let files = present
        .into_iter()
        .zip(hashes)
        .filter(|((path, _), hash)| stored_hash.get(path).and_then(|h| h.as_deref()) != Some(hash.as_str()))
        .map(|((path, content), content_hash)| EvidenceFile { path, content, content_hash })
        .collect();
    Ok(Reconcile { files, removed, revision })
}

/// Most files, and most bytes, sent as source evidence the first time.
pub const MAX_SOURCE_FILES: usize = 300;
pub const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;

/// What a repository would send as source evidence: its source, test and build files.
#[derive(Debug)]
pub struct SourceSet {
    pub files: Vec<EvidenceFile>,
    pub revision: String,
    /// Files that were candidates but did not fit the limits (too many, too much, too large, or not text).
    pub left_out: usize,
    pub bytes: usize,
}

const SOURCE_EXTENSIONS: &[&str] = &[
    "java", "kt", "rs", "py", "ts", "tsx", "js", "jsx", "go", "c", "h", "cc", "cpp", "hpp", "cs", "rb", "php", "swift",
    "scala", "sql", "sh", "toml", "xml", "gradle", "properties", "yml", "yaml", "json", "proto", "md",
];
const SOURCE_NAMES: &[&str] = &["Dockerfile", "Makefile", "pom.xml", "build.gradle", "Cargo.toml", "package.json", "go.mod"];
const SKIPPED_DIRS: &[&str] = &[
    "target/", "node_modules/", "build/", "dist/", "out/", ".git/", ".idea/", "vendor/", "reviews/", "__pycache__/", ".venv/",
];
const SKIPPED_NAMES: &[&str] = &["package-lock.json", "yarn.lock", "pnpm-lock.yaml", "Cargo.lock", "go.sum"];

/// Lower is sent first when not everything fits: build files, then main code, then tests, then the rest.
fn source_rank(path: &str) -> u8 {
    let name = path.rsplit('/').next().unwrap_or(path);
    if SOURCE_NAMES.contains(&name) {
        0
    } else if path.contains("src/main/") || path.starts_with("src/") && !path.contains("test") {
        1
    } else if path.contains("test") {
        2
    } else {
        3
    }
}

fn is_source_candidate(path: &str) -> bool {
    let padded = format!("/{path}");
    if SKIPPED_DIRS.iter().any(|d| padded.contains(&format!("/{d}"))) {
        return false;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    if SKIPPED_NAMES.contains(&name) {
        return false;
    }
    SOURCE_NAMES.contains(&name)
        || name.rsplit_once('.').map(|(_, ext)| SOURCE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())).unwrap_or(false)
}

/// The repository's source, test and build files, for the first time source evidence is sent. Honors `.gitignore`, skips
/// build output, dependencies, lock files, review reports and binary or oversized files, and stops at the limits,
/// sending build files first, then main code, then tests.
pub fn collect_source(repo: &Path) -> Result<SourceSet, String> {
    let top = top(repo)?;
    let revision = revision_label(&top)?;
    let mut paths: Vec<String> = git(&top, &["ls-files", "-z", "--cached", "--others", "--exclude-standard"])?
        .split('\0')
        .filter(|p| !p.is_empty() && is_source_candidate(p))
        .map(str::to_owned)
        .collect();
    paths.sort();
    paths.dedup();
    paths.sort_by_key(|p| (source_rank(p), p.clone()));
    let mut texts: Vec<(String, String)> = Vec::new();
    let (mut bytes, mut left_out) = (0usize, 0usize);
    for path in paths {
        match read_text(&top, &path) {
            Some(text) if texts.len() < MAX_SOURCE_FILES && bytes + text.len() <= MAX_SOURCE_BYTES => {
                bytes += text.len();
                texts.push((path, text));
            }
            _ => left_out += 1,
        }
    }
    let hashes = hash_paths(&top, &texts.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>())?;
    let files = texts
        .into_iter()
        .zip(hashes)
        .map(|((path, content), content_hash)| EvidenceFile { path, content, content_hash })
        .collect();
    Ok(SourceSet { files, revision, left_out, bytes })
}

/// One sync call's worth of files and removals.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Batch {
    pub files: Vec<EvidenceFile>,
    pub removed: Vec<String>,
}

/// Splits files and removals into calls that fit the backend's limits. There is always at least one batch, so the
/// revision is recorded even when no file changed.
pub fn batches(files: Vec<EvidenceFile>, removed: Vec<String>) -> Vec<Batch> {
    let mut out = Vec::new();
    let mut current = Batch::default();
    let mut bytes = 0usize;
    for file in files {
        if !current.files.is_empty() && (bytes + file.content.len() > MAX_BATCH_BYTES || current.files.len() >= MAX_BATCH_FILES) {
            out.push(std::mem::take(&mut current));
            bytes = 0;
        }
        bytes += file.content.len();
        current.files.push(file);
    }
    for chunk in removed.chunks(2_000) {
        if current.files.is_empty() && current.removed.len() + chunk.len() > 2_000 && !current.removed.is_empty() {
            out.push(std::mem::take(&mut current));
        }
        current.removed.extend(chunk.iter().cloned());
    }
    out.push(current);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn repo() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("meridian-mcp-change-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "t@example.test"],
            vec!["config", "user.name", "T"],
            vec!["config", "commit.gpgsign", "false"],
        ] {
            git(&dir, &args).unwrap();
        }
        dir
    }

    fn commit(dir: &Path, file: &str, text: &str, message: &str) {
        fs::write(dir.join(file), text).unwrap();
        git(dir, &["add", "-A"]).unwrap();
        git(dir, &["commit", "-q", "-m", message]).unwrap();
    }

    fn head(dir: &Path) -> String {
        git(dir, &["rev-parse", "HEAD"]).unwrap().trim().to_owned()
    }

    #[test]
    fn source_collection_takes_code_tests_and_build_files_and_skips_the_rest() {
        let dir = repo();
        fs::create_dir_all(dir.join("src/main/java")).unwrap();
        fs::create_dir_all(dir.join("src/test/java")).unwrap();
        fs::create_dir_all(dir.join("target")).unwrap();
        fs::create_dir_all(dir.join("reviews/full")).unwrap();
        fs::write(dir.join("pom.xml"), "<project/>").unwrap();
        fs::write(dir.join("src/main/java/A.java"), "class A {}").unwrap();
        fs::write(dir.join("src/test/java/ATest.java"), "class ATest {}").unwrap();
        fs::write(dir.join("target/B.java"), "class B {}").unwrap();
        fs::write(dir.join("reviews/full/r.md"), "# old review").unwrap();
        fs::write(dir.join("image.png"), [0u8, 1, 2]).unwrap();
        fs::write(dir.join("Cargo.lock"), "lock").unwrap();
        commit(&dir, "README.md", "hello", "first");
        let set = collect_source(&dir).unwrap();
        let paths: Vec<&str> = set.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["pom.xml", "src/main/java/A.java", "src/test/java/ATest.java", "README.md"]);
        assert!(set.files.iter().all(|f| f.content_hash.len() == 40));
        assert_eq!(set.left_out, 0);
    }

    #[test]
    fn source_collection_honors_gitignore_and_the_file_limit() {
        let dir = repo();
        fs::write(dir.join(".gitignore"), "secret.rs\n").unwrap();
        fs::write(dir.join("secret.rs"), "fn secret() {}").unwrap();
        for i in 0..(MAX_SOURCE_FILES + 5) {
            fs::write(dir.join(format!("f{i:04}.rs")), "fn f() {}").unwrap();
        }
        commit(&dir, "main.rs", "fn main() {}", "first");
        let set = collect_source(&dir).unwrap();
        assert!(set.files.iter().all(|f| f.path != "secret.rs"));
        assert_eq!(set.files.len(), MAX_SOURCE_FILES);
        assert_eq!(set.left_out, 6);
    }

    #[test]
    fn uncommitted_changes_are_preferred_and_their_files_come_with_them() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        fs::write(dir.join("a.txt"), "two\n").unwrap();
        let change = collect(&dir, None, None).unwrap();
        assert_eq!(change.described_as, "uncommitted changes");
        assert!(change.diff.contains("+two"));
        assert!(change.summary.is_none());
        assert_eq!(change.files.len(), 1);
        assert_eq!(change.files[0].path, "a.txt");
        assert_eq!(change.files[0].content, "two\n");
        assert_eq!(change.files[0].content_hash.len(), 40, "a git blob hash");
        assert!(change.revision.contains("+dirty."));
    }

    #[test]
    fn a_clean_tree_reviews_the_last_commit_and_names_it() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        commit(&dir, "a.txt", "two\n", "Add a cache");
        let change = collect(&dir, None, None).unwrap();
        assert_eq!(change.described_as, "the last commit");
        assert_eq!(change.summary.as_deref(), Some("Add a cache"));
        assert!(change.diff.contains("+two"));
        assert_eq!(change.revision, head(&dir));
    }

    #[test]
    fn a_base_covers_everything_since_that_ref() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        git(&dir, &["tag", "v1"]).unwrap();
        commit(&dir, "b.txt", "b\n", "second");
        commit(&dir, "c.txt", "c\n", "third");
        let change = collect(&dir, Some("v1"), None).unwrap();
        assert!(change.diff.contains("b.txt") && change.diff.contains("c.txt"));
        assert_eq!(change.described_as, "all changes since v1");
        let mut paths: Vec<_> = change.files.iter().map(|f| f.path.as_str()).collect();
        paths.sort();
        assert_eq!(paths, vec!["b.txt", "c.txt"]);
        assert!(change.base_revision.is_some());
    }

    #[test]
    fn the_evidence_revision_is_the_default_base_so_no_commit_is_skipped() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        let reviewed = head(&dir);
        commit(&dir, "b.txt", "b\n", "second");
        commit(&dir, "c.txt", "c\n", "third");
        let change = collect(&dir, None, Some(&reviewed)).unwrap();
        assert!(change.described_as.starts_with("all changes since the last reviewed revision"));
        assert!(change.diff.contains("b.txt") && change.diff.contains("c.txt"), "both unreviewed commits");
        assert_eq!(change.base_revision.as_deref(), Some(reviewed.as_str()));
    }

    #[test]
    fn a_dirty_evidence_revision_starts_from_its_commit_and_different_dirty_states_differ() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        fs::write(dir.join("a.txt"), "two\n").unwrap();
        let label = revision_label(&dir).unwrap();
        assert!(label.contains("+dirty."));

        // Still the same uncommitted state: nothing new.
        assert!(collect(&dir, None, Some(&label)).unwrap_err().contains("Nothing has changed"));

        // More uncommitted work on the same commit is a different state, and is reviewed from the commit.
        fs::write(dir.join("a.txt"), "three\n").unwrap();
        assert_ne!(revision_label(&dir).unwrap(), label);
        let change = collect(&dir, None, Some(&label)).unwrap();
        assert!(change.diff.contains("+three"));
        assert_eq!(change.base_revision.as_deref(), Some(commit_of(&label)));
    }

    #[test]
    fn a_new_untracked_file_changes_the_label() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        let clean = revision_label(&dir).unwrap();
        assert!(!clean.contains('+'));
        fs::write(dir.join("new.txt"), "x\n").unwrap();
        let with_file = revision_label(&dir).unwrap();
        fs::write(dir.join("new.txt"), "y\n").unwrap();
        assert_ne!(with_file, revision_label(&dir).unwrap());
    }

    #[test]
    fn nothing_new_since_the_reviewed_revision_is_said_plainly() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        let label = revision_label(&dir).unwrap();
        assert!(collect(&dir, None, Some(&label)).unwrap_err().contains("Nothing has changed since the last reviewed revision"));
    }

    #[test]
    fn a_revision_this_repository_does_not_have_means_history_was_rewritten() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        fs::write(dir.join("a.txt"), "two\n").unwrap();
        let change = collect(&dir, None, Some("0123456789abcdef0123456789abcdef01234567")).unwrap();
        assert!(change.base_missing);
        assert!(change.note.unwrap().contains("history was rewritten"));
        assert_eq!(change.described_as, "uncommitted changes");
    }

    #[test]
    fn deleted_files_and_new_untracked_files_are_in_the_change() {
        let dir = repo();
        commit(&dir, "keep.txt", "k\n", "first");
        commit(&dir, "gone.txt", "g\n", "second");
        fs::remove_file(dir.join("gone.txt")).unwrap();
        fs::write(dir.join("fresh.txt"), "hello\n").unwrap();
        let change = collect(&dir, None, None).unwrap();
        assert_eq!(change.removed, vec!["gone.txt"]);
        assert!(change.files.iter().any(|f| f.path == "fresh.txt" && f.content == "hello\n"));
        assert!(change.diff.contains("fresh.txt") && change.diff.contains("+hello"), "new files are in the diff too");
    }

    #[test]
    fn a_binary_or_oversized_file_cannot_be_kept_so_its_old_evidence_goes() {
        let dir = repo();
        commit(&dir, "blob.bin", "text\n", "first");
        fs::write(dir.join("blob.bin"), [0u8, 1, 2, 3]).unwrap();
        fs::write(dir.join("big.txt"), "x".repeat(MAX_FILE_BYTES + 1)).unwrap();
        let change = collect(&dir, None, None).unwrap();
        assert!(change.files.is_empty());
        assert!(change.removed.contains(&"blob.bin".to_string()));
        assert!(change.removed.contains(&"big.txt".to_string()));
    }

    #[test]
    fn nothing_to_review_and_non_repositories_are_clear_errors() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        assert!(collect(&dir, Some("HEAD"), None).unwrap_err().contains("no change to review"));
        let plain = std::env::temp_dir().join(format!("meridian-mcp-plain-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&plain).unwrap();
        assert!(collect(&plain, None, None).unwrap_err().contains("not inside a git repository"));
    }

    #[test]
    fn a_base_that_looks_like_an_option_is_refused() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        assert!(collect(&dir, Some("--output=/tmp/x"), None).unwrap_err().contains("not an option"));
    }

    #[test]
    fn an_oversized_change_is_refused_with_advice() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        fs::write(dir.join("a.txt"), "x\n".repeat(MAX_CHANGE_BYTES)).unwrap();
        let err = collect(&dir, None, None).unwrap_err();
        assert!(err.contains("at most") && err.contains("nearer `base`"));
    }

    #[test]
    fn reconcile_sends_only_stored_files_that_differ_and_names_the_ones_that_are_gone() {
        let dir = repo();
        commit(&dir, "same.txt", "same\n", "first");
        commit(&dir, "edited.txt", "old\n", "second");
        commit(&dir, "removed.txt", "x\n", "third");
        commit(&dir, "other.txt", "not evidence\n", "fourth");
        let same_hash = git(&dir, &["hash-object", "same.txt"]).unwrap().trim().to_owned();
        fs::write(dir.join("edited.txt"), "new\n").unwrap();
        fs::remove_file(dir.join("removed.txt")).unwrap();
        let stored = StoredEvidence {
            revision: Some("old".into()),
            files: vec![
                ("same.txt".into(), Some(same_hash)),
                ("edited.txt".into(), Some("stale-hash".into())),
                ("removed.txt".into(), Some("whatever".into())),
            ],
        };
        let r = reconcile(&dir, &stored).unwrap();
        assert_eq!(r.files.len(), 1);
        assert_eq!(r.files[0].path, "edited.txt");
        assert_eq!(r.files[0].content, "new\n");
        assert_eq!(r.removed, vec!["removed.txt"]);
        assert!(r.revision.contains("+dirty."));
    }

    #[test]
    fn batches_respect_the_size_and_file_limits_and_always_make_one() {
        let file = |n: usize, size: usize| EvidenceFile { path: format!("f{n}"), content: "x".repeat(size), content_hash: "h".into() };
        assert_eq!(batches(vec![], vec![]).len(), 1, "the revision is recorded even when nothing changed");

        let big: Vec<_> = (0..3).map(|n| file(n, MAX_BATCH_BYTES / 2 + 1)).collect();
        let split = batches(big, vec![]);
        assert_eq!(split.len(), 3);
        assert!(split.iter().all(|b| b.files.iter().map(|f| f.content.len()).sum::<usize>() <= MAX_BATCH_BYTES));

        let many: Vec<_> = (0..(MAX_BATCH_FILES + 1)).map(|n| file(n, 1)).collect();
        assert_eq!(batches(many, vec![]).len(), 2);
    }

    #[test]
    fn stored_evidence_is_read_from_the_backends_json() {
        let v = serde_json::json!({ "revision": "abc", "files": [ { "path": "a", "contentHash": "h1" }, { "path": "b" } ] });
        let e = StoredEvidence::from_json(&v);
        assert_eq!(e.revision.as_deref(), Some("abc"));
        assert_eq!(e.files, vec![("a".to_string(), Some("h1".to_string())), ("b".to_string(), None)]);
    }
}
