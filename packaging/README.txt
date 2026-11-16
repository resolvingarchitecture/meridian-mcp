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
- Scanning happens on your machine. Meridian receives structured observations (paths, counts,
  short summaries), not your source code.
- Your model is encrypted under a passphrase only you hold; your agent asks you for it. It is
  kept in memory only and cannot be recovered if you lose it.
- Settings: MERIDIAN_API_KEY (required), MERIDIAN_API_URL (optional, defaults to
  https://resolvingarchitecture.io/apps/meridian/api).

This build is not code-signed yet.
- macOS may refuse to open a downloaded copy. Run:  xattr -d com.apple.quarantine meridian-mcp
- Windows SmartScreen may warn. Choose "More info", then "Run anyway".
Verify the download against SHA256SUMS before you run it.
