# Manual and Help Screenshots

The Markdown tree below `docs/help` is the single source for in-application Help and the PDF and HTML manuals.

## Build the manual

Install the pinned packages in `docs/manual/requirements.txt`, then run `./build manual`. The generator validates local links and images and creates both formats from the same Markdown. The PDF has a cover, hierarchical contents, bookmarks, running headers, alternating version/page-number footers, keycaps, widow/orphan control, and alphabetical index. The HTML version is an offline single-page application with responsive hierarchical navigation, search, deep links, print styling, and inline CSS/JavaScript. Deploy `output/html/tosklight-manual-html.zip` by extracting its root-level `index.html` and `assets/` directory into a webhost document root.

## Refresh screenshots

Run `./test help-screenshots` only when intentionally updating documentation images. The serial Playwright test loads a deterministic show, drives the real browser desk, and writes the software keypad, every available pane and pane-settings dialog, and the setup-workflow gallery to `docs/help/assets/screenshots`. The test checks the exact expected filenames, so adding a pane or a documented setup surface requires an intentional coverage update. Reference the files with ordinary relative Markdown image syntax so Help, PDF, and HTML use the identical image.

The Dynamics pane remains in screenshot generation so future UI changes are detected, but its screenshot is intentionally not embedded in the manual while Dynamics is a future feature. Development remains available through developer tooling, but it is not an operator **Open Window** choice and is therefore excluded from the Pane Reference screenshot set.

## Release publication

The Forgejo manual action builds both formats on pull requests and `main`. On a trusted `v*` tag it creates or reuses the hosted release and idempotently attaches the versioned PDF and HTML deployment ZIP. Pull-request code never receives release credentials.
