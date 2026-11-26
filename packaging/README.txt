meridian-mcp: the Meridian connector for your coding agent
=========================================================

Meridian captures a project's architecture through an interview that your agent runs.
This program is the connector (an MCP server) between your agent and Meridian.

Easiest: run the installer instead of doing these steps by hand.
  Linux and macOS:  curl -fsSL https://resolvingarchitecture.io/meridian/install.sh | sh
  Windows:          irm https://resolvingarchitecture.io/meridian/install.ps1 | iex

By hand, from this package:
1. Put this program somewhere permanent (for example ~/bin, or C:\Tools).
2. Run it once:  meridian-mcp install
   It adds Meridian to the agents it finds (Claude Code, Claude Desktop, Cursor, VS Code). Only the
   "meridian" entry is added; the original file is saved next to it as <file>.meridian-backup.
   "meridian-mcp install --dry-run" shows what it would change; "--agent cursor" does one agent;
   "--remove" takes Meridian out again.
3. Or add it to your MCP client's configuration yourself:

   {
     "mcpServers": {
       "meridian": {
         "command": "/full/path/to/meridian-mcp",
         "args": ["serve"]
       }
     }
   }

   On Windows use the full path to meridian-mcp.exe, with doubled backslashes in JSON.
4. Restart your agent and ask it to set up Meridian. It walks you through signing up (a first
   purchase of at least 20 USD, paid in Bitcoin from your own wallet; or create your account in the web app and pay by card, charged in US dollars). If you already have an API
   key, add  "env": { "MERIDIAN_API_KEY": "your-api-key" }  to the same block.

What it does and does not do
- Meridian needs your project's source to review and evaluate your architecture.
- Your architecture model is encrypted at rest under a key derived from your passphrase, so
  Meridian cannot read it without that passphrase. Your agent asks you for the passphrase; the
  connector keeps it in memory only. It cannot be recovered if you lose it.
- Settings: MERIDIAN_API_KEY (optional once you have signed up through your agent), MERIDIAN_API_URL (optional, defaults to
  https://resolvingarchitecture.io/apps/meridian/api).

This build is not code-signed yet.
- macOS may refuse to open a downloaded copy. Run:  xattr -d com.apple.quarantine meridian-mcp
- Windows SmartScreen may warn. Choose "More info", then "Run anyway".
Verify the download against SHA256SUMS before you run it.
