# Marketing Screenshots

The reviewed PNG files below `assets/screenshots` are generated from deterministic
Storybook application stories. `screenshot-manifest.json` is the source contract
for their filenames, gallery order, titles, captions, story IDs, viewport sizes,
operator mode, and any deterministic interactions. Native surfaces and other desktop-only compositions use reviewed captures from
`assets/stills`, identified by `sourceFile` in the manifest. Refresh these from the
actual app before generating the final gallery; Storybook cannot render the native
Stage surface. Generated gallery images are not committed.

Run:

```sh
npm run test:marketing-screenshots
```

This builds static Storybook, captures its manifest entries, copies the native
stills, and refreshes `assets/screenshots`. Capture candidates also appear under
`.artifacts/test/marketing-screenshots/storybook`. This is a content and rendering
gate, not a pixel comparison against a baseline.

Recreate the complete reviewed gallery with the root command:

```sh
npm run screenshots:marketing
```

Visually inspect the generated images against their titles and captions.

The capture gate fixes the clock, waits for Storybook's documentation-ready
signal and web fonts, disables animation, rejects live REST or WebSocket access,
checks image dimensions and rendered content, and verifies library thumbnails decode.
All generated scratch files and reports stay within the canonical `.artifacts`
tree.

The public landing-page gallery consumes this manifest and these reviewed images
directly. CI runs `npm run screenshots:marketing`, uploads the generated
`marketing-screenshots` artifact, and downloads that exact artifact before
assembling the public Pages site. Marketing screenshots remain separate from
`docs/help/screenshot-manifest.json`; Help and manual images are refreshed through
their own manifest. Help can also preserve real native Stage stills; it never
substitutes an unavailable browser canvas for a Stage illustration.

On pushes to `main`, the Pages assembly job downloads that generated artifact and,
immediately beside its GitHub Pages artifact upload, uploads the same PNG files to
S3. Configure these GitHub Actions secrets:

- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`
- `AWS_REGION`
- `MARKETING_SCREENSHOTS_S3_BUCKET` — bucket name only, without `s3://`
- `MARKETING_SCREENSHOTS_S3_BASE_PATH` — a non-root prefix within the bucket

`AWS_SESSION_TOKEN` is supported as an optional secret for temporary credentials.
The upload copies PNG files into the configured prefix without deleting any other
S3 objects.
