use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    let git_hash = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let git_dirty = Command::new("git")
        .args(["diff", "--quiet"])
        .status()
        .ok()
        .map(|status| if status.success() { "false" } else { "true" })
        .unwrap_or("unknown");

    let build_timestamp = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=MERIDIAN_GIT_HASH={git_hash}");
    println!("cargo:rustc-env=MERIDIAN_GIT_DIRTY={git_dirty}");
    println!("cargo:rustc-env=MERIDIAN_BUILD_TIMESTAMP={build_timestamp}");
}
