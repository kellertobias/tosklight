# Manual and Help Screenshots

The Markdown tree below `docs/help` is the single source for in-application Help and the PDF and HTML manuals.

## Build the manual

Run `./build manual`. The command creates an isolated Python environment under ignored `.artifacts/cache/` and automatically installs the pinned packages from `docs/help/.tooling/requirements.txt` when the environment is missing, incomplete, or the manifest changes. No separate `pip install` step is required. The generator validates local links and images and creates both formats from the same Markdown. The PDF has a cover, hierarchical contents, bookmarks, running headers, alternating version/page-number footers, keycaps, widow/orphan control, and alphabetical index. The HTML version is an offline single-page application with responsive hierarchical navigation, search, deep links, print styling, and inline CSS/JavaScript. Deploy `.artifacts/generated/manual/html/tosklight-manual-html.zip` by extracting its root-level `index.html` and `assets/` directory into a webhost document root.

The stable outputs are:

- `.artifacts/generated/manual/pdf/tosklight-manual.pdf` — the paginated print manual.
- `.artifacts/generated/manual/html/tosklight-manual/index.html` — the offline single-page HTML manual.
- `.artifacts/generated/manual/html/tosklight-manual-html.zip` — the webhost-ready archive containing root-level `index.html` and referenced images under `assets/`.

## Refresh screenshots

`docs/help/screenshot-manifest.json` is the complete source contract for the PNG files under
`docs/help/assets/screenshots`. Every tracked PNG has exactly one entry containing its stable
filename, source, viewport, dark theme, software/hardware mode, and deterministic interaction list.
Storybook-owned entries name an existing application story. A screenshot that cannot yet be
represented truthfully has `source: "live-app"`, a null story ID, and an explicit reason.

Run `npm run test:help-screenshots` to build static Storybook and check the reviewed screenshots
serially without launching Light or opening a mutable show. The gate rejects incomplete manifests,
missing story IDs, live REST/WebSocket traffic, console or page errors, blank captures, dimension
drift, and pixel differences above the review threshold. Candidate images are always written below
`.artifacts/test/help-screenshots/storybook`.

After inspecting those candidates, run `npm run test:help-screenshots:update` to replace only the
Storybook-owned files in `docs/help/assets/screenshots`, inspect every Git image diff, and then rerun
the check command. Filenames remain unchanged so in-app Help, PDF, HTML manual, and Pages all consume
the same reviewed assets.

`npm run test:help-screenshots-live` is the separately named real-app path. It retains the smaller
set of screenshots still marked `live-app` in the manifest, including pane-settings and
setup/fixture-library/MVR workflow surfaces that do not yet have honest deterministic application
stories. The command still drives the production browser desk and server; Storybook-owned captures
from that run go only to `.artifacts/test/help-screenshots/live-app` and cannot overwrite their
reviewed documentation files.

Dynamics pool/editor screenshots are refreshed only after an explicit user review of the production
UI. This prevents component tests and pixel references from freezing an unaccepted interaction or
layout. Until that checkpoint is complete, keep the existing Dynamics image and manifest ownership
unchanged; documentation text may describe the implemented operator contract without presenting an
unreviewed replacement image.

## Refresh icon contact sheets

Run `npm run icons:contact-sheets` after changing an editable SVG below `assets/icons`. The generator first writes a filled-geometry `name.expanded.svg` beside every `name.svg`, then renders one deterministic PNG per icon group plus a complete-library sheet under `.artifacts/generated/icon-contact-sheets`. Existing `.expanded.svg` files are never treated as sources. Each group sheet has a 90-degree group label on the left, a vertical divider, and the group's black icons and editable source filenames on the right.

The expanded derivative resolves strokes, transforms, repeated patterns, and binary mounting-gap masks into filled `currentColor` geometry on a transparent canvas. A final Boolean union joins overlapping painted shapes into one compound path per icon. Do not edit it directly. The generator mirrors the PNGs into the ignored `docs/help/assets/icon-contact-sheets` directory so in-app Help, manual, and Pages builds can package them without adding contact sheets to Git. These builds regenerate and verify the source, expanded-SVG, and generated-PNG hashes before rendering.

## Authoring contract

- Put operator-facing source in a numbered file or folder below `docs/help`.
- Give every page exactly one first-level `# Title`; it becomes the Help navigation title, contents entry, running header, and index entry.
- Use ordinary relative Markdown links and images. The manual build fails for broken local links or images.
- Keep screenshots under `docs/help/assets/screenshots`, add every PNG to
  `docs/help/screenshot-manifest.json`, and use a real application story whenever one exists.
- Never assign a convenient but inaccurate story ID. Keep unmatched workflow images on the
  explicitly documented live-app path until an equivalent application story exists.
- Refresh Storybook-owned images with `npm run test:help-screenshots:update`; use
  `npm run test:help-screenshots-live` only for the remaining live-app entries.
- Keep generated expanded icons beside their editable sources. Contact sheets belong under `.artifacts/generated/icon-contact-sheets` with only an ignored Help mirror; regenerate both through `npm run icons:contact-sheets`.
- Add or update the matching row in [Help Coverage](02-help-coverage.md) when introducing a built-in window or major operator workflow.

## Release publication

The CI screenshot job builds static Storybook, checks the reviewed manifest, and uploads one
`help-screenshots` artifact. The manual job downloads that exact artifact before rendering PDF and
HTML, and the Pages job downloads the same artifact before assembling the public site. GitHub
deploys the complete Pages site from mirrored `main`; pull-request code receives no release or
deployment credentials.
