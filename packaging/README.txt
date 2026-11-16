meridian-mcp: the Meridian connector for your coding agent
=========================================================

Meridian captures a project's architecture through an interview that your agent runs.
This program is the connector (an MCP server) between your agent and Meridian.

1. Put this program somewhere permanent (for example ~/bin, or C:\Tools).
2. Create a Meridian API key in the Meridian web app and keep it secret.
3. Add the connector to your MCP client's configuration:

   {
     "mcpServers": {
       "meridian": {
         "command": "/full/path/to/meridian-mcp",
         "args": ["serve"],
         "env": { "MERIDIAN_API_KEY": "your-api-key" }
       }
     }
   }

   On Windows use the full path to meridian-mcp.exe, with doubled backslashes in JSON.
4. Restart your agent and ask it to list your Meridian projects.

What it does and does not do
- Meridian needs your project's source to review and evaluate your architecture.
- Your architecture model is encrypted at rest under a key derived from your passphrase, so
  Meridian cannot read it without that passphrase. Your agent asks you for the passphrase; the
  connector keeps it in memory only. It cannot be recovered if you lose it.
- Settings: MERIDIAN_API_KEY (required), MERIDIAN_API_URL (optional, defaults to
  https://resolvingarchitecture.io/apps/meridian/api).

This build is not code-signed yet.
- macOS may refuse to open a downloaded copy. Run:  xattr -d com.apple.quarantine meridian-mcp
- Windows SmartScreen may warn. Choose "More info", then "Run anyway".
Verify the download against SHA256SUMS before you run it.
