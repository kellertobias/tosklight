# PDF and HTML manual build

The operator manual is generated entirely from `docs/help`. Those Markdown files and their relative images remain the source of truth for the in-application Help window, professionally typeset PDF, and deployable HTML manual.

## Build locally

```sh
python3 -m pip install -r docs/manual/requirements.txt
./build manual
```

The stable outputs are:

- `output/pdf/tosklight-manual.pdf` — the paginated print manual.
- `output/html/tosklight-manual/index.html` — the single-page web application with all CSS, JavaScript, navigation, search, and manual text in one HTML file.
- `output/html/tosklight-manual-html.zip` — the deployment archive containing root-level `index.html` and only the referenced images below `assets/`.

The HTML manual has no CDN or runtime dependency. Extract the ZIP directly into a webhost document root. It supports hash/deep-link navigation, browser history, responsive navigation, full-text page filtering, print styling, and the same desk/keyboard keycap distinction as Help and the PDF.

The build validates internal Markdown links, missing images, duplicate headings, PDF metadata and pagination, HTML anchors and offline resources, safe ZIP paths, and an exact match between the verified site and deployment archive.

## Refresh application screenshots

```sh
./test help-screenshots
./build manual
```

The screenshot test opens a deterministic seeded show, drives the real browser UI, and writes publication images to `docs/help/assets/screenshots`. Help pages reference those files with ordinary relative Markdown image links, so the same image appears in Help, PDF, and HTML. Run the screenshot command only when intentionally updating documentation images; normal test suites do not rewrite committed screenshots.

## Authoring contract

- Put operator-facing source in a numbered file or folder below `docs/help`.
- Give every page exactly one first-level `# Title`; it becomes the Help navigation title, contents entry, running header, and index entry.
- Use normal relative Markdown links and images. The manual build fails for broken local links or images.
- Keep screenshots under `docs/help/assets/screenshots` and generate them through the Playwright documentation test. Its exact-filename gate covers every entry in the Open Window registry, every pane-settings dialog, and the documented desk/show setup workflows. Dynamics is generated for future-regression coverage but intentionally not printed while it is a future feature.
- Add a row to `docs/help/99-Development/02-help-coverage.md` when introducing a new built-in window or major operator workflow.

The generator deliberately supports the Markdown constructs used by Help: headings, paragraphs, links, emphasis, colored vector desk buttons (`[AT]`, `[1]`, `[CLR]`, and `[REC]`), visually distinct computer-keyboard keys (`[KBD:ENTER]`), lists, tables, block quotes, fenced code, horizontal rules, and standalone images.

## Forgejo release assets

`.forgejo/workflows/manual.yml` builds and verifies both formats on pull requests and pushes to `main`. A trusted `v*` tag build creates the hosted Forgejo release when necessary, preserves an existing release, and idempotently attaches `tosklight-manual-vVERSION.pdf` plus `tosklight-manual-vVERSION-html.zip`. Publication uses the configured `FORGEJO_TOKEN` secret and requires an `ubuntu-latest` runner with Python 3. No publication runs for pull-request code.
