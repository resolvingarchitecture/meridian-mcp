//! Saves a finished review as a markdown report.
//!
//! The backend returns the review as markdown (`report`). The connector runs next to the repository,
//! so it is the part that can read the project's version and write the file. It picks the destination
//! so the user need not type one: inside a git repository the report goes in that repository's
//! `meridian/`; across several repositories it goes in the `meridian/` of an architecture, design, or
//! admin folder. When neither applies it is saved in a scratch folder, so nothing is lost, and the
//! agent offers to move it. Every report records the project version at the time of the review in its
//! front matter and in its file name, so reports for different versions sit side by side.

use serde_json::Value;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Where the project's version came from.
#[derive(Debug, PartialEq, Eq)]
pub struct DetectedVersion {
    pub version: String,
    pub source: &'static str,
}

/// Reads the project version from the first manifest in `root` that has one: `pom.xml`,
/// `package.json`, `Cargo.toml`, `pyproject.toml`.
pub fn detect_version(root: &Path) -> Option<DetectedVersion> {
    let read = |name: &str| fs::read_to_string(root.join(name)).ok();
    if let Some(text) = read("pom.xml") {
        if let Some(v) = pom_version(&text) {
            return Some(DetectedVersion { version: v, source: "pom.xml" });
        }
    }
    if let Some(text) = read("package.json") {
        if let Some(v) = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|j| j.get("version").and_then(Value::as_str).map(str::to_owned))
        {
            return Some(DetectedVersion { version: v, source: "package.json" });
        }
    }
    if let Some(text) = read("Cargo.toml") {
        if let Some(v) = toml_version(&text, &["package"]) {
            return Some(DetectedVersion { version: v, source: "Cargo.toml" });
        }
    }
    if let Some(text) = read("pyproject.toml") {
        if let Some(v) = toml_version(&text, &["project", "tool.poetry"]) {
            return Some(DetectedVersion { version: v, source: "pyproject.toml" });
        }
    }
    None
}

/// The project's own `<version>`: the first one outside `<parent>`, which comes before the
/// dependencies by Maven convention.
fn pom_version(text: &str) -> Option<String> {
    let mut rest = text.to_owned();
    while let (Some(start), Some(end)) = (rest.find("<parent>"), rest.find("</parent>")) {
        if end < start {
            break;
        }
        rest.replace_range(start..end + "</parent>".len(), "");
    }
    let start = rest.find("<version>")? + "<version>".len();
    let end = rest[start..].find("</version>")? + start;
    let v = rest[start..end].trim();
    (!v.is_empty() && !v.contains("${")).then(|| v.to_owned())
}

fn toml_version(text: &str, sections: &[&str]) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            in_section = sections.contains(&name.trim());
            continue;
        }
        if in_section {
            if let Some(value) = line.strip_prefix("version") {
                let value = value.trim_start();
                if let Some(value) = value.strip_prefix('=') {
                    let v = value.trim().trim_matches(|c| c == '"' || c == '\'');
                    if !v.is_empty() && !v.starts_with('{') {
                        return Some(v.to_owned());
                    }
                }
            }
        }
    }
    None
}

/// What was saved.
#[derive(Debug)]
pub struct Saved {
    pub path: PathBuf,
    pub version: String,
    pub version_source: String,
}

/// Where a report goes without being told: the `meridian/` folder it belongs in, and why.
#[derive(Debug, PartialEq, Eq)]
pub struct Destination {
    pub dir: PathBuf,
    pub reason: String,
}

/// Folder-name keywords, in priority order, that mark the place reports for several repositories belong.
const HOME_FOLDERS: [&str; 3] = ["architecture", "design", "admin"];

/// Picks the `meridian/` folder for work under `start`:
/// - inside a git repository: that repository's `meridian/`;
/// - otherwise, if `start` holds git repositories (the work spans several): the `meridian/` of the child
///   folder whose name contains `architecture`, then `design`, then `admin` (ties by name);
/// - otherwise nothing, and the caller uses the scratch folder.
pub fn resolve_destination(start: &Path) -> Option<Destination> {
    let start = fs::canonicalize(start).ok()?;
    for dir in start.ancestors() {
        if dir.join(".git").exists() {
            return Some(Destination {
                dir: dir.join("meridian"),
                reason: format!("inside the git repository {}", dir.display()),
            });
        }
    }
    let children: Vec<PathBuf> = fs::read_dir(&start)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    if !children.iter().any(|c| c.join(".git").exists()) {
        return None;
    }
    let mut best: Option<(usize, String, PathBuf)> = None;
    for child in children {
        let name = child.file_name()?.to_string_lossy().to_lowercase();
        if name.starts_with('.') {
            continue;
        }
        if let Some(rank) = HOME_FOLDERS.iter().position(|k| name.contains(k)) {
            if best.as_ref().is_none_or(|(r, n, _)| (rank, &name) < (*r, n)) {
                best = Some((rank, name, child));
            }
        }
    }
    best.map(|(_, name, dir)| Destination {
        dir: dir.join("meridian"),
        reason: format!("{} holds several repositories; reports go in its \"{name}\" folder", start.display()),
    })
}

/// The scratch folder reports are saved in first: `MERIDIAN_REVIEW_SCRATCH`, else
/// `$XDG_CACHE_HOME/meridian/reviews`, else `~/.cache/meridian/reviews`, else the system temp folder.
pub fn scratch_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MERIDIAN_REVIEW_SCRATCH").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    if let Some(cache) = std::env::var_os("XDG_CACHE_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(cache).join("meridian").join("reviews");
    }
    if let Some(home) = std::env::var_os("HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(home).join(".cache").join("meridian").join("reviews");
    }
    std::env::temp_dir().join("meridian-reviews")
}

/// Moves a report out of `scratch` into `to_dir` and returns its new path. Only a file inside `scratch`
/// can be moved (this is not a general file mover), and an existing file is never overwritten.
pub fn move_from_scratch(scratch: &Path, from: &Path, to_dir: &Path) -> Result<PathBuf, String> {
    let scratch = fs::canonicalize(scratch).map_err(|e| format!("scratch folder {}: {e}", scratch.display()))?;
    let from = fs::canonicalize(from).map_err(|e| format!("{}: {e}", from.display()))?;
    if !from.starts_with(&scratch) || !from.is_file() {
        return Err(format!("{} is not a saved review in the scratch folder {}", from.display(), scratch.display()));
    }
    if to_dir.exists() && !to_dir.is_dir() {
        return Err(format!("{} is not a directory", to_dir.display()));
    }
    fs::create_dir_all(to_dir).map_err(|e| format!("create {}: {e}", to_dir.display()))?;
    let to = fs::canonicalize(to_dir)
        .map_err(|e| format!("{}: {e}", to_dir.display()))?
        .join(from.file_name().ok_or("the file has no name")?);
    let content = fs::read(&from).map_err(|e| format!("read {}: {e}", from.display()))?;
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&to)
        .map_err(|e| format!("write {}: {e}", to.display()))?;
    out.write_all(&content).map_err(|e| format!("write {}: {e}", to.display()))?;
    fs::remove_file(&from).map_err(|e| format!("copied to {} but could not remove {}: {e}", to.display(), from.display()))?;
    Ok(to)
}

/// What a report would be called and which version it records, before anything is written.
#[derive(Debug)]
pub struct Plan {
    pub file_name: String,
    pub version: String,
    pub version_source: String,
    reviewed_at: String,
    service: String,
    evaluation_id: String,
}

/// Works out the file name and project version for a completed evaluation result, from the manifest in
/// `repo_root` unless `version_override` is given. Writes nothing.
pub fn plan(repo_root: &Path, version_override: Option<&str>, result: &Value) -> Result<Plan, String> {
    result
        .get("report")
        .and_then(Value::as_str)
        .filter(|r| !r.trim().is_empty())
        .ok_or("the result has no report to save")?;
    let (version, source) = match version_override.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => (v.to_owned(), "provided".to_owned()),
        None => match fs::canonicalize(repo_root).ok().and_then(|r| detect_version(&r)) {
            Some(d) => (d.version, d.source.to_owned()),
            None => ("unknown".to_owned(), "not found".to_owned()),
        },
    };
    let version: String = version.chars().filter(|c| !c.is_control()).collect();

    let text = |k: &str| result.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    let service = text("service");
    let evaluation_id = text("evaluationId");
    let reviewed_at = match text("evaluatedAt") {
        t if !t.is_empty() => t,
        _ => format!("epoch-{}", SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())),
    };
    let stamp = file_stamp(&reviewed_at);
    let file_name = format!("{}-{}-v{}.md", sanitize(&stamp), sanitize(&service.to_lowercase()), sanitize(&version));
    Ok(Plan { file_name, version, version_source: source, reviewed_at, service, evaluation_id })
}

/// The time of the review as a file-name prefix that sorts by time: `2026-11-17T19-06-45Z` for a full UTC time,
/// or just the date when that is all there is. Colons are replaced because some filesystems refuse them.
fn file_stamp(reviewed_at: &str) -> String {
    let chars: Vec<char> = reviewed_at.chars().collect();
    let looks_like_time = chars.len() >= 19 && chars[10] == 'T';
    if !looks_like_time {
        return chars.iter().take(10).collect();
    }
    let base: String = chars[..19].iter().map(|&c| if c == ':' { '-' } else { c }).collect();
    if reviewed_at.ends_with('Z') { format!("{base}Z") } else { base }
}

/// Writes the report into `dest_dir` (created if needed) and returns where it went, or why not. Never
/// overwrites an existing file.
pub fn save(dest_dir: &Path, repo_root: &Path, version_override: Option<&str>, project: &str, result: &Value) -> Result<Saved, String> {
    let plan = plan(repo_root, version_override, result)?;
    let report = result.get("report").and_then(Value::as_str).unwrap_or_default();
    if dest_dir.exists() && !dest_dir.is_dir() {
        return Err(format!("{} is not a directory", dest_dir.display()));
    }
    fs::create_dir_all(dest_dir).map_err(|e| format!("create {}: {e}", dest_dir.display()))?;
    let dest = fs::canonicalize(dest_dir).map_err(|e| format!("{}: {e}", dest_dir.display()))?;
    let path = dest.join(&plan.file_name);

    let model_version = result.get("modelVersion").map(|v| v.to_string()).unwrap_or_default();
    let content = format!(
        "---\nservice: {}\nproject_version: {}\nversion_source: {}\nreviewed_at: {}\n\
         architecture_model_version: {model_version}\nproject_id: {project}\nevaluation_id: {}\n---\n\n{report}\n",
        plan.service, plan.version, plan.version_source, plan.reviewed_at, plan.evaluation_id
    );
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    file.write_all(content.as_bytes()).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(Saved { path, version: plan.version, version_source: plan.version_source })
}

/// Keeps a file-name part to letters, digits, dot, underscore, and hyphen.
fn sanitize(part: &str) -> String {
    let s: String = part
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' })
        .collect();
    if s.is_empty() { "x".to_owned() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("meridian-mcp-{name}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn pom_version_skips_the_parent_block() {
        let pom = "<project><parent><groupId>g</groupId><version>9.9.9</version></parent>\
                   <artifactId>a</artifactId><version>1.3.1</version>\
                   <dependencies><dependency><version>2.0</version></dependency></dependencies></project>";
        assert_eq!(pom_version(pom).as_deref(), Some("1.3.1"));
        assert_eq!(pom_version("<project><version>${rev}</version></project>"), None);
    }

    #[test]
    fn toml_version_reads_only_the_named_section() {
        let toml = "[dependencies]\nserde = { version = \"1\" }\n[package]\nname = \"x\"\nversion = \"0.1.1\"\n";
        assert_eq!(toml_version(toml, &["package"]).as_deref(), Some("0.1.1"));
        assert_eq!(toml_version("[package]\nversion.workspace = true\n", &["package"]), None);
    }

    #[test]
    fn detects_a_version_from_package_json() {
        let dir = temp("pkg");
        fs::write(dir.join("package.json"), r#"{"name":"x","version":"2.4.0"}"#).unwrap();
        let d = detect_version(&dir).unwrap();
        assert_eq!((d.version.as_str(), d.source), ("2.4.0", "package.json"));
    }

    #[test]
    fn saves_a_markdown_report_with_the_version_and_never_overwrites() {
        let dir = temp("save");
        fs::write(dir.join("pom.xml"), "<project><version>1.3.1</version></project>").unwrap();
        let result = json!({
            "service": "REVIEW_FULL", "evaluationId": "abcdef12-0000", "evaluatedAt": "2026-11-17T01:00:00Z",
            "modelVersion": 6, "report": "# Review\n\nFindings."
        });

        let saved = save(&dir.join("meridian"), &dir, None, "proj-1", &result).unwrap();

        assert_eq!(saved.version, "1.3.1");
        assert!(saved.path.ends_with("meridian/2026-11-17T01-00-00Z-review_full-v1.3.1.md"), "{:?}", saved.path);
        let text = fs::read_to_string(&saved.path).unwrap();
        assert!(text.starts_with("---\nservice: REVIEW_FULL\nproject_version: 1.3.1\nversion_source: pom.xml\n"));
        assert!(text.contains("architecture_model_version: 6"));
        assert!(text.ends_with("# Review\n\nFindings.\n"));
        assert!(save(&dir.join("meridian"), &dir, None, "proj-1", &result).is_err(), "must not overwrite");
    }

    #[test]
    fn the_file_name_carries_the_utc_time_and_falls_back_to_the_date() {
        assert_eq!(file_stamp("2026-11-17T19:06:45.680684490Z"), "2026-11-17T19-06-45Z");
        assert_eq!(file_stamp("2026-11-17T19:06:45"), "2026-11-17T19-06-45");
        assert_eq!(file_stamp("2026-11-17"), "2026-11-17");
    }

    #[test]
    fn an_explicit_version_wins_and_a_missing_one_is_recorded_as_unknown() {
        let dir = temp("ver");
        let result = json!({ "service": "REVIEW_FULL", "evaluationId": "id1", "evaluatedAt": "2026-11-17", "report": "r" });
        assert_eq!(save(&dir, &dir, Some("7.0.0"), "p", &result).unwrap().version, "7.0.0");
        let dir2 = temp("none");
        assert_eq!(save(&dir2, &dir2, None, "p", &result).unwrap().version, "unknown");
    }

    #[test]
    fn plan_writes_nothing_and_a_file_destination_is_refused() {
        let dir = temp("plan");
        let result = json!({ "service": "REVIEW_FULL", "evaluationId": "abcdef12", "evaluatedAt": "2026-11-17T01:00:00Z", "report": "r" });
        let planned = plan(&dir, Some("1.0.0"), &result).unwrap();
        assert_eq!(planned.file_name, "2026-11-17T01-00-00Z-review_full-v1.0.0.md");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0, "plan must not write");

        let file = dir.join("not-a-dir");
        fs::write(&file, "x").unwrap();
        assert!(save(&file, &dir, Some("1.0.0"), "p", &result).unwrap_err().contains("not a directory"));
    }

    #[test]
    fn a_saved_report_moves_out_of_scratch_but_nothing_else_does() {
        let scratch = temp("scratch");
        let dest = temp("dest");
        let result = json!({ "service": "REVIEW_FULL", "evaluationId": "abcdef12", "evaluatedAt": "2026-11-17", "report": "r" });
        let saved = save(&scratch, &dest, Some("1.0.0"), "p", &result).unwrap();

        let moved = move_from_scratch(&scratch, &saved.path, &dest.join("meridian")).unwrap();

        assert!(moved.ends_with("meridian/2026-11-17-review_full-v1.0.0.md"), "{moved:?}");
        assert!(!saved.path.exists(), "the scratch copy is gone");
        assert!(fs::read_to_string(&moved).unwrap().contains("project_version: 1.0.0"));

        // A file outside scratch cannot be moved, and an existing destination is never overwritten.
        let other = dest.join("other.txt");
        fs::write(&other, "x").unwrap();
        assert!(move_from_scratch(&scratch, &other, &dest.join("meridian")).unwrap_err().contains("not a saved review"));
        let again = save(&scratch, &dest, Some("1.0.0"), "p", &result).unwrap();
        assert!(move_from_scratch(&scratch, &again.path, &dest.join("meridian")).is_err());
        assert!(again.path.exists(), "a refused move leaves the scratch file alone");
    }

    #[test]
    fn a_git_repository_gets_its_own_meridian_folder() {
        let repo = temp("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("src/deep")).unwrap();

        let d = resolve_destination(&repo.join("src/deep")).unwrap();

        assert_eq!(d.dir, fs::canonicalize(&repo).unwrap().join("meridian"));
        assert!(d.reason.contains("git repository"));
    }

    #[test]
    fn several_repositories_use_an_architecture_design_or_admin_folder() {
        let root = temp("multi");
        for name in ["svc-a", "svc-b", "zeta-admin", "my-design-docs", "notes"] {
            fs::create_dir_all(root.join(name)).unwrap();
        }
        fs::create_dir_all(root.join("svc-a/.git")).unwrap();
        fs::create_dir_all(root.join("svc-b/.git")).unwrap();

        let d = resolve_destination(&root).unwrap();
        assert_eq!(d.dir, fs::canonicalize(&root).unwrap().join("my-design-docs/meridian"), "design beats admin");

        fs::create_dir_all(root.join("platform-architecture")).unwrap();
        let d = resolve_destination(&root).unwrap();
        assert_eq!(d.dir, fs::canonicalize(&root).unwrap().join("platform-architecture/meridian"), "architecture wins");
    }

    #[test]
    fn nothing_matches_means_no_destination() {
        let root = temp("none");
        fs::create_dir_all(root.join("svc/.git")).unwrap();
        assert_eq!(resolve_destination(&root), None, "repositories but no architecture/design/admin folder");
        let empty = temp("empty");
        assert_eq!(resolve_destination(&empty), None, "no repositories at all");
    }

    #[test]
    fn nothing_is_saved_without_a_report_or_with_a_hostile_version() {
        let dir = temp("bad");
        assert!(save(&dir, &dir, None, "p", &json!({"status": "NOT_READY"})).is_err());
        let result = json!({ "service": "S", "evaluationId": "i", "evaluatedAt": "2026-11-17", "report": "r" });
        let saved = save(&dir.join("meridian"), &dir, Some("../../etc/x"), "p", &result).unwrap();
        assert!(saved.path.starts_with(dir.canonicalize().unwrap().join("meridian")), "{:?}", saved.path);
    }
}