---
apply: always
---

---
apply: always
---

# AI Assistant Rules

## Edit snippet filename requirements

When suggesting edits to existing project files, every applyable edit snippet must identify its target file in two ways:

1. A visible Markdown filename line for the user.
2. A hidden/apply filename tag for tooling.

The visible filename line is required because the UI may hide the `<llm-snippet-file>` tag from the user.

Use this exact order for every applyable edit snippet:

```markdown
**File: `path/to/file.ext`**

<llm-snippet-file>path/to/file.ext</llm-snippet-file>
```
```language
// code here
```

Rules:

- Always put the visible filename line immediately before the `<llm-snippet-file>` tag.
- Always put the `<llm-snippet-file>` tag immediately before the fenced code block.
- The `<llm-snippet-file>` tag must be outside the fenced code block, never inside it.
- Do not rely only on the `<llm-snippet-file>` tag to identify the target file.
- Do not rely on prose paragraphs, tables, headings, or previous mentions to identify the target file.
- Do not omit either filename indicator just because the file was already mentioned earlier.
- If multiple snippets edit the same file, repeat both the visible filename line and the `<llm-snippet-file>` tag before each snippet.
- If multiple files are edited, each file’s snippet must have its own visible filename line and matching `<llm-snippet-file>` tag.
- Before sending any response that contains an applyable edit snippet, verify that every applyable fenced code block has both:
    - a directly preceding visible filename line, and
    - a directly preceding `<llm-snippet-file>` tag.
- Non-edit examples, commands, logs, and explanatory snippets do not require filename lines or `<llm-snippet-file>` tags.

## Code edit snippet rules

When suggesting edits for existing source files:

- Prefer grouping all edits for a file in a single snippet.
- Snippets must show changed lines with minimal surrounding unchanged lines for context.
- Include enough context to make the edit unambiguous.
- Use comments such as `// ... existing code ...`, `# ... existing code ...`, or `/* ... existing code ... */` to indicate omitted unchanged code.
- Do not omit spans of existing code without explicitly marking the omission.
- Do not use diff-style markers such as `+ line` or `- line`.
- Always specify the programming language in fenced code blocks.

## Markdown edit application rules

Fenced code blocks are allowed and preferred in Markdown documents when they improve readability.

When editing an existing Markdown file:

- Provide one applyable snippet per file whenever possible.
- If the edit includes fenced code blocks, include the whole affected section or the whole file in a single applyable snippet so it can be applied with one click.
- Do not split Markdown edits into many individually applyable fenced blocks.
- Preserve language identifiers on fenced code blocks inside Markdown content.
- Prefer replacing a complete section, such as from one `##` heading to the next `##` heading, when the section contains fenced code blocks.
- If many Markdown code fences are involved, provide the full updated file content as one applyable snippet.
- When providing a full Markdown file replacement that itself contains fenced code blocks, wrap the entire applyable snippet in a higher-order fence, such as four backticks with `markdown`, so inner triple-backtick fences do not break the single apply.
- A "single apply" means one visible filename line, one file tag, one outer fenced block, and no additional applyable code fences for that same file outside the outer fenced block.
- For Markdown files with embedded examples in multiple languages, use exactly one outer apply fence for the file and keep all inner example fences inside it.
- Optimize Markdown file edits for one-click apply while keeping the final Markdown document readable.
- Always specify the language for fenced code blocks.

## Markdown table formatting

When writing Markdown tables:

- Tables must be visually aligned.
- Pad table cells with spaces so column separators line up.
- Use separator rows with enough hyphens to match the column width.
- Avoid compact table separators like `|---|---|`.

Preferred table style:

```markdown
| File                      | Purpose                                                            |
|---------------------------|--------------------------------------------------------------------|
| `docker-compose.yml`      | Local development stack, including Meridian services               |
| `docker-compose.prod.yml` | Production-oriented stack, excluding local-only infrastructure     |
```

## Java generation

- Records used by multiple packages across features should be placed in the `models` package.
- Records used only within one package should be placed in that package.
- Do not generate records inside a class unless the only references to the record are within that class.
- If any code outside of a class references a record, place the record in the appropriate package instead of nesting it inside the class.