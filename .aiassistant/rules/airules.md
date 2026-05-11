---
apply: always
---

---
apply: always
---

## Markdown edit application rules

- Fenced code blocks are allowed and preferred in Markdown documents when they improve readability.
- When editing an existing Markdown file, provide one applyable snippet per file whenever possible.
- If the edit includes fenced code blocks, include the whole affected section or the whole file in a single applyable snippet so it can be applied with one click.
- Do not split Markdown edits into many individually applyable fenced blocks.
- Preserve language identifiers on fenced code blocks inside Markdown content.
- Prefer replacing a complete section, such as from one `##` heading to the next `##` heading, when the section contains fenced code blocks.
- If many Markdown code fences are involved, provide the full updated file content as one applyable snippet.
- When providing a full Markdown file replacement that itself contains fenced code blocks, wrap the entire applyable snippet in a higher-order fence, such as four backticks with `markdown`, so inner triple-backtick fences do not break the single apply.
- A "single apply" means one file tag, one outer fenced block, and no additional applyable code fences for that same file outside the outer fenced block.
- For Markdown files with embedded examples in multiple languages, use exactly one outer apply fence for the file and keep all inner example fences inside it.
- Optimize Markdown file edits for one-click apply while keeping the final Markdown document readable.
- Always specify the language for fenced code blocks.
- Tables must be visually aligned.
- Pad table cells with spaces so column separators line up.
- Use separator rows with enough hyphens to match the column width.
- Avoid compact table separators like `|---|---|`.

Example of the preferred single-apply style for Markdown files containing nested code fences:

`````markdown
<llm-snippet-file>path/to/file.md</llm-snippet-file>
````markdown
# Title

Text before an inner code block.

```bash
echo "This inner fence stays inside the one outer apply fence."
```

More Markdown content.
````
`````

Example of the preferred table style:

| File                      | Purpose                                                                   |
|---------------------------| ------------------------------------------------------------------------- |
| `docker-compose.yml`      | Local development stack, including GitLab CE and Meridian services        |
| `docker-compose.prod.yml` | Production-oriented stack, excluding GitLab and local-only infrastructure |