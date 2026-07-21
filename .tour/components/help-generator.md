---
slug: help-generator
title: Help Generator
summary: "docs/help Markdown as the single source for in-app help, the PDF manual, and the HTML manual."
order: 60
---

# Help and Manual Generator

`docs/help/` Markdown is the source for both the in-app Help window and the PDF and HTML manuals.
There is no second copy to keep in sync.

Authoring contract: `docs/help/99-Development/04-manual-and-help-screenshots.md`.

## Source layout

```
docs/help/
  00-quickstart.markdown
  01-application-layout.md
  02-installation.md
  05-Pane-Reference/     {index.md, 01..03}
  10-Desk-Setup/         {index.md, 01..04}
  20-Show-Setup/         {index.md, 01..05}
  30-Programmer/         {index.md, 01..05}
  40-Running-a-Show/     {index.md, 01..05}
  50-Protocols/          {index.md, 01-osc-rest-and-websocket.md}
  99-Development/        {index.md, ...}
  assets/screenshots/    {*.png, panes/, workflows/}
  .tooling/requirements.txt
```

Build-enforced rules:

- Exactly one `# Title` per page, and the first H1 must work as the visible title.
- Numbered file and folder prefixes set the ordering; `index.md` sorts as `!index`.
- Relative Markdown links and images. Broken local targets fail the build.
- Duplicate bookmarks are rejected.

## In-app help

`crates/server/src/help.rs` embeds `docs/help` with `#[derive(RustEmbed)]` and serves:

- `GET /api/v1/help` — catalog tree
- `GET /api/v1/help/topics/{*id}`
- `GET /api/v1/help/assets/{*path}`

Debug builds prefer a live on-disk `docs/help` (`live_help_dir()`), so edits show up immediately.
Release builds serve the embedded copy.

Frontend: `apps/control-ui/src/windows/HelpWindow.tsx`, `helpMarkdown.ts`,
`apps/control-ui/src/api/client/help.ts`, `apps/control-ui/src/help.css`.

## ./build manual

Provisions a pinned Python venv at `.artifacts/cache/manual-venv` from
`docs/help/.tooling/requirements.txt` (reportlab, markdown-it-py, pypdf, pdfplumber), then runs:

1. `tools/build_manual.py` → PDF
2. `tools/verify_manual.py` → validate the PDF
3. `tools/build_html_manual.py` → HTML site plus deployable ZIP
4. `tools/verify_html_manual.py` → validate site and archive

### tools/manual/

| File | Role |
| --- | --- |
| `config.py` | Paths, palette, A4 geometry, `register_fonts()`, ReportLab styles, `workspace_version()` |
| `source.py` | Page discovery and ordering, one-H1 validation, duplicate-bookmark detection, link and image target validation |
| `markdown.py` | Markdown to ReportLab flowables |
| `keycaps.py` | Renders `[REC]`-style keycap images |
| `template.py` | Cover, running headers, alternating version and page footers |
| `build_pdf.py` | Document assembly, bookmarks, hierarchical contents, index |

`tools/build_html_manual.py` uses markdown-it-py and Pillow to emit an offline single-page app with
inline CSS and JS, search, deep links, and print styles.

### Verification

`tools/verify_manual.py` checks the PDF exists, is at least 100 KB and 20 pages, carries the title
metadata `ToskLight Operator Manual`, contains the required section titles in order, has an outline,
and has the expected image count.

### Outputs

```
.artifacts/generated/manual/pdf/tosklight-manual.pdf
.artifacts/generated/manual/html/tosklight-manual/index.html
.artifacts/generated/manual/html/tosklight-manual-html.zip
```

`.forgejo/workflows/manual.yml` builds on PR and main; on `v*` tags it attaches both artifacts via
`tools/publish_forgejo_manual.py`. PR builds receive no credentials.

## ./test help-screenshots

Wipes and regenerates the images, so run it only when refreshing them.

`tests/02-help-screenshots.spec.ts` (serial, 180 s, 1600×1100) seeds the default-stage show, drives
the real desk, and regenerates:

- `docs/help/assets/screenshots/*.png` — keypad, desk overview, cuelist/playback, fixture sheet,
  command line
- `.../panes/` — `<slug>.png` per pane, plus `<slug>-settings.png` where a settings tab exists
- `.../workflows/` — show menu, load and revisions, change user, MVR import and export, patch, add
  fixture, desk setup and lock, fixture library, stage 2D and settings, fixture-sheet settings

The spec computes expected pane files from the `paneReference` table and asserts the directory
listing matches exactly, so adding a pane forces a screenshot update rather than leaving a gap.

`playwright.config.ts` excludes the spec from normal runs unless `LIGHT_HELP_SCREENSHOTS=1`.

Review screenshot diffs visually and keep them tied to stable, representative operator states.

## Workflow for a help change

```sh
# edit docs/help/**.md
./dev                      # debug server serves live help
./build manual             # PDF and HTML build plus verification
./test help-screenshots    # only if images need refreshing
```

## Read first

1. `docs/help/99-Development/04-manual-and-help-screenshots.md`
2. `crates/server/src/help.rs`
3. `tools/manual/source.py`
4. `tools/verify_manual.py`
5. `tests/02-help-screenshots.spec.ts`
