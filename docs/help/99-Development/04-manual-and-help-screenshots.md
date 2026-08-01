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
`docs/help/assets/screenshots`. Every declared PNG has exactly one entry containing its stable
filename, source, viewport, dark theme, software/hardware mode, and deterministic interaction list.
Storybook-owned entries name an existing application story. A screenshot that cannot yet be
represented truthfully has `source: "live-app"`, a null story ID, and an explicit reason.

The screenshots are generated, not reviewed, and `docs/help/assets/screenshots` is not in Git. CI
captures the whole set on every run and the release build embeds it, so what ships is always the
current interface. A screenshot cannot go stale, because none is ever kept.

Two captures fill the directory. `npm run test:help-screenshots` builds static Storybook and takes
the entries a story can show, without launching Light or opening a mutable show.
`npm run test:help-screenshots-live` drives the real desk and server for the entries marked
`live-app` — pane settings and the setup, fixture-library and MVR workflows that have no honest
deterministic story. `npm run screenshots:help` runs both, which is what you want locally: a fresh
clone has no help images until you do.

Each capture rejects incomplete manifests, missing story IDs, live REST/WebSocket traffic, console
or page errors, blank captures, and dimension drift. Those checks are what make an unreviewed
capture safe to ship. Neither compares against a previous run: visual regression is a separate
concern and deliberately not part of this. Because neither capture can see the other's output,
`node tools/check-help-screenshot-set.mjs` verifies the assembled directory against the manifest,
and CI runs it before anything consumes the set.

Filenames stay stable, so in-app Help, the PDF, the HTML manual and Pages all consume the same
generated assets.

## Refresh icon contact sheets

Run `npm run icons:contact-sheets` after changing an editable SVG below `assets/icons`. The generator first writes a filled-geometry `name.expanded.svg` beside every `name.svg`, then renders one deterministic PNG per icon group plus a complete-library sheet under `.artifacts/generated/icon-contact-sheets`. Existing `.expanded.svg` files are never treated as sources. Each group sheet has a 90-degree group label on the left, a vertical divider, and the group's black icons and editable source filenames on the right.

The expanded derivative resolves strokes, transforms, repeated patterns, and binary mounting-gap masks into filled `currentColor` geometry on a transparent canvas. A final Boolean union joins overlapping painted shapes into one compound path per icon. Do not edit it directly. The generator mirrors the PNGs into the ignored `docs/help/assets/icon-contact-sheets` directory so in-app Help, manual, and Pages builds can package them without adding contact sheets to Git. These builds regenerate and verify the source, expanded-SVG, and generated-PNG hashes before rendering.

## Authoring contract

- Put operator-facing source in a numbered file or folder below `docs/help`.
- Give every page exactly one first-level `# Title`; it becomes the Help navigation title, contents entry, running header, and index entry.
- Use ordinary relative Markdown links and images. The manual build fails for broken local links or images.
- Declare every screenshot in `docs/help/screenshot-manifest.json` and use a real application
  story whenever one exists. The manifest is the declaration; `docs/help/assets/screenshots` is
  generated output and is not committed.
- Never assign a convenient but inaccurate story ID. Keep unmatched workflow images on the
  explicitly documented live-app path until an equivalent application story exists.
- Regenerate locally with `npm run screenshots:help`, which runs both captures. CI does the same
  on every run, so there is nothing to refresh by hand.
- Keep generated expanded icons beside their editable sources. Contact sheets belong under `.artifacts/generated/icon-contact-sheets` with only an ignored Help mirror; regenerate both through `npm run icons:contact-sheets`.
- Add or update the matching row in [Help Coverage](02-help-coverage.md) when introducing a built-in window or major operator workflow.

## Release publication

Two CI jobs capture the halves — `documentation-screenshots` from static Storybook and
`help-screenshots-live` against the prebuilt Playwright server — and `help-screenshots` assembles
them, verifies the set against the manifest, and publishes one `help-screenshots` artifact. The
release build, the manual and the Pages job all download that artifact: the build because the
images are embedded into the binary, the other two before rendering. GitHub deploys the complete
Pages site from mirrored `main`; pull-request code receives no release or deployment credentials.

The marketing gallery is published to the S3 preview prefix on every run, together with the product
demo video, so both can be looked at without being shipped. Promoting the preview to the prefix the
website serves is a manual `workflow_dispatch` with **Promote marketing** ticked, and nothing else
does it.
