//! `meridian-mcp install`: adds Meridian to the MCP configuration of the coding agents found on this machine, so nobody
//! has to find and hand-edit a config file. Only the `meridian` entry is touched; every other setting and server in the
//! file is kept as it was (including the order of keys), the original is saved once as `<file>.meridian-backup`, and a file
//! that is not valid JSON is left alone and reported.

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

const SERVER: &str = "meridian";

/// How an agent's file lays out its servers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    /// `{ "mcpServers": { "name": { "command": ..., "args": [...] } } }` (Claude Code, Claude Desktop, Cursor).
    McpServers,
    /// `{ "servers": { "name": { "type": "stdio", "command": ..., "args": [...] } } }` (VS Code).
    VsCode,
}

/// Where an agent keeps its configuration, and what must exist for the agent to count as installed.
#[derive(Clone, Debug)]
struct Target {
    name: &'static str,
    /// The agent's configuration file.
    file: PathBuf,
    shape: Shape,
    /// The file is created when this directory exists but the file does not; when false, the file itself must exist.
    create_when_dir_exists: bool,
}

/// The parts of the environment that decide where the files are, so tests can supply their own.
#[derive(Clone, Debug)]
pub struct Env {
    pub home: PathBuf,
    /// Windows only: `%APPDATA%`.
    pub appdata: Option<PathBuf>,
    pub os: &'static str,
}

impl Env {
    pub fn current() -> Result<Env> {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
            .context("cannot tell where your home folder is (HOME is not set)")?;
        Ok(Env { home, appdata: std::env::var_os("APPDATA").map(PathBuf::from), os: std::env::consts::OS })
    }

    /// The folder holding per-user application settings for this platform.
    fn app_settings(&self) -> PathBuf {
        match self.os {
            "macos" => self.home.join("Library").join("Application Support"),
            "windows" => self.appdata.clone().unwrap_or_else(|| self.home.join("AppData").join("Roaming")),
            _ => self.home.join(".config"),
        }
    }
}

fn targets(env: &Env) -> Vec<Target> {
    let settings = env.app_settings();
    vec![
        Target { name: "Claude Code", file: env.home.join(".claude.json"), shape: Shape::McpServers, create_when_dir_exists: false },
        Target { name: "Claude Desktop", file: settings.join("Claude").join("claude_desktop_config.json"), shape: Shape::McpServers, create_when_dir_exists: true },
        Target { name: "Cursor", file: env.home.join(".cursor").join("mcp.json"), shape: Shape::McpServers, create_when_dir_exists: true },
        Target { name: "VS Code", file: settings.join("Code").join("User").join("mcp.json"), shape: Shape::VsCode, create_when_dir_exists: true },
    ]
}

/// What happened for one agent.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Added,
    Updated,
    Unchanged,
    Removed,
    NotThere,
    NotInstalled,
    Skipped(String),
}

pub struct Report {
    pub agent: &'static str,
    pub file: PathBuf,
    pub outcome: Outcome,
}

/// Adds (or, with `remove`, takes out) the `meridian` entry for every agent found, or only `only` when given.
pub fn run(env: &Env, command: &Path, only: Option<&str>, remove: bool, dry_run: bool) -> Result<Vec<Report>> {
    let all = targets(env);
    if let Some(name) = only {
        let wanted = normalise(name);
        if !all.iter().any(|t| normalise(t.name) == wanted) {
            bail!("unknown agent {name:?}; use one of: {}", all.iter().map(|t| t.name).collect::<Vec<_>>().join(", "));
        }
    }
    let mut reports = Vec::new();
    for target in all {
        if only.is_some_and(|name| normalise(name) != normalise(target.name)) {
            continue;
        }
        let outcome = apply(&target, command, remove, dry_run);
        reports.push(Report { agent: target.name, file: target.file.clone(), outcome });
    }
    Ok(reports)
}

fn normalise(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

fn apply(target: &Target, command: &Path, remove: bool, dry_run: bool) -> Outcome {
    let present = target.file.is_file();
    let dir_exists = target.file.parent().is_some_and(Path::is_dir);
    if !present && !(target.create_when_dir_exists && dir_exists) {
        return Outcome::NotInstalled;
    }
    match edit(target, command, remove, dry_run, present) {
        Ok(outcome) => outcome,
        Err(e) => Outcome::Skipped(format!("{e:#}")),
    }
}

fn entry(shape: Shape, command: &Path, existing: Option<&Value>) -> Value {
    let mut map = Map::new();
    if shape == Shape::VsCode {
        map.insert("type".into(), json!("stdio"));
    }
    map.insert("command".into(), json!(command.to_string_lossy()));
    map.insert("args".into(), json!(["serve"]));
    // Keep what the user added to the entry, such as an API key in "env".
    if let Some(Value::Object(old)) = existing {
        for (key, value) in old {
            if !map.contains_key(key) {
                map.insert(key.clone(), value.clone());
            }
        }
    }
    Value::Object(map)
}

fn edit(target: &Target, command: &Path, remove: bool, dry_run: bool, present: bool) -> Result<Outcome> {
    let key = match target.shape {
        Shape::McpServers => "mcpServers",
        Shape::VsCode => "servers",
    };
    let original = if present { Some(std::fs::read_to_string(&target.file).with_context(|| format!("cannot read {}", target.file.display()))?) } else { None };
    let mut root: Value = match &original {
        Some(text) if !text.trim().is_empty() => serde_json::from_str(text).context("the file is not valid JSON, so it was left alone")?,
        _ => Value::Object(Map::new()),
    };
    let Some(object) = root.as_object_mut() else { bail!("the file does not hold a JSON object, so it was left alone") };

    let outcome = if remove {
        let removed = object.get_mut(key).and_then(Value::as_object_mut).and_then(|servers| servers.shift_remove(SERVER));
        if removed.is_some() { Outcome::Removed } else { return Ok(Outcome::NotThere) }
    } else {
        let servers = object.entry(key).or_insert_with(|| Value::Object(Map::new()));
        let Some(servers) = servers.as_object_mut() else { bail!("{key:?} in the file is not an object, so it was left alone") };
        let new = entry(target.shape, command, servers.get(SERVER));
        match servers.get(SERVER) {
            Some(old) if *old == new => return Ok(Outcome::Unchanged),
            Some(_) => {
                servers.insert(SERVER.into(), new);
                Outcome::Updated
            }
            None => {
                servers.insert(SERVER.into(), new);
                Outcome::Added
            }
        }
    };

    if dry_run {
        return Ok(outcome);
    }
    if let Some(parent) = target.file.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("cannot create {}", parent.display()))?;
    }
    if let Some(text) = &original {
        let backup = PathBuf::from(format!("{}.meridian-backup", target.file.display()));
        if !backup.exists() {
            std::fs::write(&backup, text).with_context(|| format!("cannot save a backup at {}", backup.display()))?;
        }
    }
    let temp = PathBuf::from(format!("{}.meridian-tmp", target.file.display()));
    let mut text = serde_json::to_string_pretty(&root)?;
    text.push('\n');
    std::fs::write(&temp, text).with_context(|| format!("cannot write {}", temp.display()))?;
    std::fs::rename(&temp, &target.file).with_context(|| format!("cannot replace {}", target.file.display()))?;
    Ok(outcome)
}

/// The lines `install` prints.
pub fn describe(reports: &[Report], remove: bool, dry_run: bool) -> String {
    let mut out = String::new();
    let would = if dry_run { "would be " } else { "" };
    for r in reports {
        let line = match &r.outcome {
            Outcome::Added => format!("{}: Meridian {would}added ({})", r.agent, r.file.display()),
            Outcome::Updated => format!("{}: Meridian {would}updated to this program ({})", r.agent, r.file.display()),
            Outcome::Unchanged => format!("{}: already set up", r.agent),
            Outcome::Removed => format!("{}: Meridian {would}removed ({})", r.agent, r.file.display()),
            Outcome::NotThere => format!("{}: Meridian was not set up", r.agent),
            Outcome::NotInstalled => format!("{}: not found on this machine", r.agent),
            Outcome::Skipped(why) => format!("{}: skipped ({why}; {})", r.agent, r.file.display()),
        };
        out.push_str(&line);
        out.push('\n');
    }
    let changed = reports.iter().any(|r| matches!(r.outcome, Outcome::Added | Outcome::Updated | Outcome::Removed));
    let any_found = reports.iter().any(|r| !matches!(r.outcome, Outcome::NotInstalled));
    if !any_found && !remove {
        out.push_str("\nNo supported agent was found. Add this to your agent's MCP configuration by hand:\n  command: this program's full path\n  args: [\"serve\"]\n");
    } else if changed && !dry_run {
        out.push_str(if remove { "\nRestart your agent to finish.\n" } else { "\nRestart your agent, then ask it to set up Meridian.\n" });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("meridian-install-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn env(home: &Path) -> Env {
        Env { home: home.to_path_buf(), appdata: None, os: "linux" }
    }

    fn exe() -> PathBuf {
        PathBuf::from("/opt/meridian/meridian-mcp")
    }

    fn desktop(home: &Path) -> PathBuf {
        home.join(".config/Claude/claude_desktop_config.json")
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn outcome(reports: &[Report], agent: &str) -> String {
        format!("{:?}", reports.iter().find(|r| r.agent == agent).unwrap().outcome)
    }

    #[test]
    fn it_adds_meridian_and_keeps_everything_else_in_place() {
        let h = home();
        write(&desktop(&h), r#"{ "theme": "dark", "mcpServers": { "other": { "command": "x" } }, "zeta": 1 }"#);
        let reports = run(&env(&h), &exe(), None, false, false).unwrap();

        assert_eq!(outcome(&reports, "Claude Desktop"), "Added");
        let text = std::fs::read_to_string(desktop(&h)).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["mcpServers"]["meridian"]["command"], "/opt/meridian/meridian-mcp");
        assert_eq!(v["mcpServers"]["meridian"]["args"], json!(["serve"]));
        assert_eq!(v["mcpServers"]["other"]["command"], "x", "another server is untouched");
        assert_eq!(v["theme"], "dark");
        assert!(text.find("theme").unwrap() < text.find("mcpServers").unwrap() && text.find("mcpServers").unwrap() < text.find("zeta").unwrap(),
                "the order of the user's keys is kept");
    }

    #[test]
    fn running_it_twice_changes_nothing_the_second_time() {
        let h = home();
        write(&desktop(&h), "{}");
        run(&env(&h), &exe(), None, false, false).unwrap();
        let first = std::fs::read_to_string(desktop(&h)).unwrap();
        let reports = run(&env(&h), &exe(), None, false, false).unwrap();

        assert_eq!(outcome(&reports, "Claude Desktop"), "Unchanged");
        assert_eq!(std::fs::read_to_string(desktop(&h)).unwrap(), first);
    }

    #[test]
    fn a_moved_program_updates_the_path_and_keeps_the_users_api_key() {
        let h = home();
        write(&desktop(&h), r#"{ "mcpServers": { "meridian": { "command": "/old/path", "args": ["serve"], "env": { "MERIDIAN_API_KEY": "k" } } } }"#);
        let reports = run(&env(&h), &exe(), None, false, false).unwrap();

        assert_eq!(outcome(&reports, "Claude Desktop"), "Updated");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(desktop(&h)).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["meridian"]["command"], "/opt/meridian/meridian-mcp");
        assert_eq!(v["mcpServers"]["meridian"]["env"]["MERIDIAN_API_KEY"], "k");
    }

    #[test]
    fn the_original_file_is_saved_once_and_never_overwritten_by_a_later_run() {
        let h = home();
        let original = r#"{"mcpServers":{}}"#;
        write(&desktop(&h), original);
        run(&env(&h), &exe(), None, false, false).unwrap();
        run(&env(&h), Path::new("/somewhere/else"), None, false, false).unwrap();

        let backup = PathBuf::from(format!("{}.meridian-backup", desktop(&h).display()));
        assert_eq!(std::fs::read_to_string(backup).unwrap(), original, "the backup is the file as it was before Meridian");
    }

    #[test]
    fn a_file_that_is_not_json_is_left_alone_and_reported() {
        let h = home();
        write(&desktop(&h), "{ this is not json");
        let reports = run(&env(&h), &exe(), None, false, false).unwrap();

        assert!(outcome(&reports, "Claude Desktop").starts_with("Skipped"), "{}", outcome(&reports, "Claude Desktop"));
        assert_eq!(std::fs::read_to_string(desktop(&h)).unwrap(), "{ this is not json");
    }

    #[test]
    fn vs_code_uses_its_own_layout() {
        let h = home();
        write(&h.join(".config/Code/User/mcp.json"), "{}");
        run(&env(&h), &exe(), None, false, false).unwrap();

        let v: Value = serde_json::from_str(&std::fs::read_to_string(h.join(".config/Code/User/mcp.json")).unwrap()).unwrap();
        assert_eq!(v["servers"]["meridian"]["type"], "stdio");
        assert_eq!(v["servers"]["meridian"]["command"], "/opt/meridian/meridian-mcp");
        assert!(v.get("mcpServers").is_none());
    }

    #[test]
    fn an_agent_that_is_not_installed_is_skipped_and_nothing_is_created_for_it() {
        let h = home();
        write(&desktop(&h), "{}");
        let reports = run(&env(&h), &exe(), None, false, false).unwrap();

        assert_eq!(outcome(&reports, "Cursor"), "NotInstalled");
        assert_eq!(outcome(&reports, "Claude Code"), "NotInstalled", "Claude Code needs its own config file to exist");
        assert!(!h.join(".cursor").exists());
        assert!(!h.join(".claude.json").exists());
    }

    #[test]
    fn an_agent_whose_folder_exists_gets_its_file_created() {
        let h = home();
        std::fs::create_dir_all(h.join(".cursor")).unwrap();
        let reports = run(&env(&h), &exe(), None, false, false).unwrap();

        assert_eq!(outcome(&reports, "Cursor"), "Added");
        assert!(h.join(".cursor/mcp.json").is_file());
    }

    #[test]
    fn a_dry_run_reports_what_it_would_do_and_writes_nothing() {
        let h = home();
        write(&desktop(&h), "{}");
        let reports = run(&env(&h), &exe(), None, false, true).unwrap();

        assert_eq!(outcome(&reports, "Claude Desktop"), "Added");
        assert_eq!(std::fs::read_to_string(desktop(&h)).unwrap(), "{}");
        assert!(!PathBuf::from(format!("{}.meridian-backup", desktop(&h).display())).exists());
        assert!(describe(&reports, false, true).contains("would be added"));
    }

    #[test]
    fn remove_takes_out_only_meridian() {
        let h = home();
        write(&desktop(&h), r#"{ "mcpServers": { "other": { "command": "x" }, "meridian": { "command": "y" } } }"#);
        let reports = run(&env(&h), &exe(), None, true, false).unwrap();

        assert_eq!(outcome(&reports, "Claude Desktop"), "Removed");
        let v: Value = serde_json::from_str(&std::fs::read_to_string(desktop(&h)).unwrap()).unwrap();
        assert!(v["mcpServers"].get("meridian").is_none());
        assert_eq!(v["mcpServers"]["other"]["command"], "x");
        assert_eq!(outcome(&run(&env(&h), &exe(), None, true, false).unwrap(), "Claude Desktop"), "NotThere");
    }

    #[test]
    fn one_agent_can_be_chosen_by_name_and_a_wrong_name_is_an_error() {
        let h = home();
        write(&desktop(&h), "{}");
        std::fs::create_dir_all(h.join(".cursor")).unwrap();
        let reports = run(&env(&h), &exe(), Some("claude-desktop"), false, false).unwrap();

        assert_eq!(reports.len(), 1);
        assert!(!h.join(".cursor/mcp.json").exists());
        assert!(run(&env(&h), &exe(), Some("emacs"), false, false).is_err());
    }

    #[test]
    fn windows_and_macos_use_their_own_settings_folders() {
        let h = home();
        let mac = Env { home: h.clone(), appdata: None, os: "macos" };
        assert_eq!(targets(&mac)[1].file, h.join("Library/Application Support/Claude/claude_desktop_config.json"));
        let win = Env { home: h.clone(), appdata: Some(h.join("AppData/Roaming")), os: "windows" };
        assert_eq!(targets(&win)[1].file, h.join("AppData/Roaming/Claude/claude_desktop_config.json"));
    }

    #[test]
    fn nothing_found_says_how_to_do_it_by_hand() {
        let h = home();
        let reports = run(&env(&h), &exe(), None, false, false).unwrap();
        assert!(describe(&reports, false, false).contains("No supported agent was found"));
    }
}
