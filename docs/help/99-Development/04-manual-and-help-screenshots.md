# Manual and Help Screenshots

The Markdown tree below `docs/help` is the single source for in-application Help and the PDF and HTML manuals.

## Build the manual

Run `./build manual`. The command creates an isolated Python environment under ignored `.artifacts/cache/` and automatically installs the pinned packages from `docs/help/.tooling/requirements.txt` when the environment is missing, incomplete, or the manifest changes. No separate `pip install` step is required. The generator validates local links and images and creates both formats from the same Markdown. The PDF has a cover, hierarchical contents, bookmarks, running headers, alternating version/page-number footers, keycaps, widow/orphan control, and alphabetical index. The HTML version is an offline single-page application with responsive hierarchical navigation, search, deep links, print styling, and inline CSS/JavaScript. Deploy `.artifacts/generated/manual/html/tosklight-manual-html.zip` by extracting its root-level `index.html` and `assets/` directory into a webhost document root.

The stable outputs are:

- `.artifacts/generated/manual/pdf/tosklight-manual.pdf` — the paginated print manual.
- `.artifacts/generated/manual/html/tosklight-manual/index.html` — the offline single-page HTML manual.
- `.artifacts/generated/manual/html/tosklight-manual-html.zip` — the webhost-ready archive containing root-level `index.html` and referenced images under `assets/`.

## Refresh screenshots

Run `./test help-screenshots` only when intentionally updating documentation images. The serial Playwright test loads a deterministic show, drives the real browser desk, and writes the software keypad, every available pane and pane-settings dialog, and the setup-workflow gallery to `docs/help/assets/screenshots`. The test checks the exact expected filenames, so adding a pane or a documented setup surface requires an intentional coverage update. Reference the files with ordinary relative Markdown image syntax so Help, PDF, and HTML use the identical image.

The Dynamics pane remains in screenshot generation so future UI changes are detected, but its screenshot is intentionally not embedded in the manual while Dynamics is a future feature. Development remains available through developer tooling, but it is not an operator **Open Window** choice and is therefore excluded from the Pane Reference screenshot set.

## Authoring contract

- Put operator-facing source in a numbered file or folder below `docs/help`.
- Give every page exactly one first-level `# Title`; it becomes the Help navigation title, contents entry, running header, and index entry.
- Use ordinary relative Markdown links and images. The manual build fails for broken local links or images.
- Keep screenshots under `docs/help/assets/screenshots` and generate them through `./test help-screenshots`.
- Add or update the matching row in [Help Coverage](02-help-coverage.md) when introducing a built-in window or major operator workflow.

## Release publication

The Forgejo manual action builds both formats on pull requests and `main`. On a trusted `v*` tag it creates or reuses the hosted release and idempotently attaches the versioned PDF and HTML deployment ZIP. Pull-request code never receives release credentials.
