//! The change an intermediate review looks at: a unified git diff collected from the local repository.
//! The diff is sent to the backend for the review and is never written to disk by this client.

use std::path::Path;
use std::process::Command;

/// Largest diff sent for one review; the backend refuses more.
pub const MAX_CHANGE_BYTES: usize = 512 * 1024;

#[derive(Debug)]
pub struct Change {
    /// A unified diff.
    pub diff: String,
    /// What the change is, in a few words: the last commit's subject when the change is a commit.
    pub summary: Option<String>,
    /// Which range the diff covers, for the caller to show.
    pub described_as: String,
}

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Collects the change to review.
/// - With `base`: everything since that git ref, committed or not.
/// - Without it: the uncommitted changes (staged and unstaged); when there are none, the last commit.
pub fn collect(repo: &Path, base: Option<&str>) -> Result<Change, String> {
    git(repo, &["rev-parse", "--is-inside-work-tree"])
        .map_err(|_| format!("{} is not inside a git repository, so there is no change to review.", repo.display()))?;

    let (diff, described_as, summary) = match base.map(str::trim).filter(|b| !b.is_empty()) {
        Some(base) => {
            if base.starts_with('-') {
                return Err("base must be a git ref (a branch, tag, or commit), not an option.".into());
            }
            let diff = git(repo, &["diff", "--no-color", "--no-ext-diff", base, "--"])
                .map_err(|e| format!("could not diff against {base}: {e}"))?;
            (diff, format!("all changes since {base}"), None)
        }
        None => {
            let working = git(repo, &["diff", "--no-color", "--no-ext-diff", "HEAD", "--"]).unwrap_or_default();
            if !working.trim().is_empty() {
                (working, "uncommitted changes".to_string(), None)
            } else {
                let last = git(repo, &["show", "--no-color", "--no-ext-diff", "--format=", "HEAD", "--"])
                    .map_err(|e| format!("there are no uncommitted changes and no commit to review: {e}"))?;
                let subject = git(repo, &["log", "-1", "--format=%s"]).ok().map(|s| s.trim().to_string());
                (last, "the last commit".to_string(), subject.filter(|s| !s.is_empty()))
            }
        }
    };

    if diff.trim().is_empty() {
        return Err(format!("There is no change to review ({described_as} is empty)."));
    }
    if diff.len() > MAX_CHANGE_BYTES {
        return Err(format!(
            "The change ({described_as}) is {} KiB; one review takes at most {} KiB. Review a smaller range, for example with a nearer `base`.",
            diff.len() / 1024,
            MAX_CHANGE_BYTES / 1024
        ));
    }
    Ok(Change { diff, summary, described_as })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

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

    #[test]
    fn uncommitted_changes_are_preferred() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        fs::write(dir.join("a.txt"), "two\n").unwrap();
        let change = collect(&dir, None).unwrap();
        assert_eq!(change.described_as, "uncommitted changes");
        assert!(change.diff.contains("+two"));
        assert!(change.summary.is_none());
    }

    #[test]
    fn a_clean_tree_reviews_the_last_commit_and_names_it() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        commit(&dir, "a.txt", "two\n", "Add a cache");
        let change = collect(&dir, None).unwrap();
        assert_eq!(change.described_as, "the last commit");
        assert_eq!(change.summary.as_deref(), Some("Add a cache"));
        assert!(change.diff.contains("+two"));
    }

    #[test]
    fn a_base_covers_everything_since_that_ref() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        git(&dir, &["tag", "v1"]).unwrap();
        commit(&dir, "b.txt", "b\n", "second");
        commit(&dir, "c.txt", "c\n", "third");
        let change = collect(&dir, Some("v1")).unwrap();
        assert!(change.diff.contains("b.txt") && change.diff.contains("c.txt"));
        assert_eq!(change.described_as, "all changes since v1");
    }

    #[test]
    fn nothing_to_review_and_non_repositories_are_clear_errors() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        // The only commit has content; diffing a base equal to HEAD is empty.
        assert!(collect(&dir, Some("HEAD")).unwrap_err().contains("no change to review"));
        let plain = std::env::temp_dir().join(format!("meridian-mcp-plain-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&plain).unwrap();
        assert!(collect(&plain, None).unwrap_err().contains("not inside a git repository"));
    }

    #[test]
    fn a_base_that_looks_like_an_option_is_refused() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        assert!(collect(&dir, Some("--output=/tmp/x")).unwrap_err().contains("not an option"));
    }

    #[test]
    fn an_oversized_change_is_refused_with_advice() {
        let dir = repo();
        commit(&dir, "a.txt", "one\n", "first");
        fs::write(dir.join("big.txt"), "x\n".repeat(MAX_CHANGE_BYTES)).unwrap();
        git(&dir, &["add", "-A"]).unwrap();
        let err = collect(&dir, None).unwrap_err();
        assert!(err.contains("at most") && err.contains("nearer `base`"));
    }
}
