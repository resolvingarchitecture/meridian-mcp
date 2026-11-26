#!/usr/bin/env python3
"""Builds the Claude Desktop bundle (.mcpb) for one platform from a meridian-mcp binary.

An .mcpb is a zip with a manifest.json at its root and the server under server/; double-clicking it in Claude Desktop
installs the connector without a terminal. Each bundle holds one platform's binary, so there is one per platform.

    python3 packaging/mcpb.py --version 0.10.1 --label windows-x86_64 \
        --binary target/x86_64-pc-windows-msvc/release/meridian-mcp.exe --license LICENSE --out dist/x.mcpb

Only the standard library is used, so it runs on every build runner as is.
"""
import argparse
import json
import sys
import zipfile

DESCRIPTION = ("Your coding agent interviews your project for the architecture Meridian is missing, reviews the design and the "
               "code, and tracks a score and a task list that improve as you fix things.")

TOOLS = [
    ("meridian_setup", "Sign up and buy credits: Bitcoin from your own wallet, or by email, then your API key."),
    ("meridian_account", "Show your credit balance, what each review costs, and your reviews so far."),
    ("meridian_projects", "List your Meridian projects or create one."),
    ("meridian_next", "Send what your agent found, answer the questions Meridian asks, and see what is still missing."),
    ("meridian_submit", "Record facts about the project directly."),
    ("meridian_model", "Read the architecture model, its completeness and open questions."),
    ("meridian_evaluate", "Run a full review or an intermediate review (a check after a commit) and save the report in your repository."),
]

# Claude Desktop names its platforms the way Node does.
PLATFORMS = {"windows": "win32", "macos": "darwin", "linux": "linux"}


def platform_of(label):
    name = label.split("-", 1)[0]
    if name not in PLATFORMS:
        sys.exit(f"unknown platform in label {label!r}; expected one of {sorted(PLATFORMS)}-<arch>")
    return PLATFORMS[name]


def manifest(version, label):
    windows = label.startswith("windows")
    exe = "meridian-mcp.exe" if windows else "meridian-mcp"
    return {
        "manifest_version": "0.2",
        "name": "meridian",
        "display_name": "Meridian",
        "version": version,
        "description": "Architecture capture and review for your coding agent.",
        "long_description": DESCRIPTION + " Add it, then ask your agent to set up Meridian: it walks you through signing up "
                            "(a first purchase of at least 20 USD, paid in Bitcoin from your own wallet), or you can create your account in the web app and pay by card. Your architecture "
                            "model is encrypted under a passphrase only you know.",
        "author": {"name": "Resolving Architecture", "url": "https://resolvingarchitecture.io"},
        "homepage": "https://resolvingarchitecture.io/meridian/",
        "documentation": "https://resolvingarchitecture.io/meridian/",
        "support": "https://resolvingarchitecture.io/contact/?topic=Meridian",
        "license": "MIT",
        "keywords": ["architecture", "review", "mcp", "code-review", "adr"],
        "server": {
            "type": "binary",
            "entry_point": f"server/{exe}",
            "mcp_config": {"command": "${__dirname}/server/" + exe, "args": ["serve"]},
        },
        "tools": [{"name": n, "description": d} for n, d in TOOLS],
        "compatibility": {"platforms": [platform_of(label)]},
    }


def build(version, label, binary, license_path, out):
    windows = label.startswith("windows")
    exe = "meridian-mcp.exe" if windows else "meridian-mcp"
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        text = zipfile.ZipInfo("manifest.json", date_time=(2026, 1, 1, 0, 0, 0))
        text.compress_type = zipfile.ZIP_DEFLATED
        text.external_attr = (0o100644) << 16
        z.writestr(text, json.dumps(manifest(version, label), indent=2) + "\n")
        # The binary keeps its executable bit: the zip records Unix permissions in the high 16 bits of external_attr.
        info = zipfile.ZipInfo(f"server/{exe}", date_time=(2026, 1, 1, 0, 0, 0))
        info.compress_type = zipfile.ZIP_DEFLATED
        info.external_attr = (0o100755) << 16
        with open(binary, "rb") as f:
            z.writestr(info, f.read())
        z.write(license_path, "LICENSE")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--version", required=True)
    ap.add_argument("--label", required=True, help="windows-x86_64, macos-arm64, macos-x86_64 or linux-x86_64")
    ap.add_argument("--binary", required=True)
    ap.add_argument("--license", required=True)
    ap.add_argument("--out", required=True)
    a = ap.parse_args()
    build(a.version, a.label, a.binary, a.license, a.out)
    print(a.out)


if __name__ == "__main__":
    main()
