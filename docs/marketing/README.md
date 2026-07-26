# Marketing Screenshots

The reviewed PNG files below `assets/screenshots` are generated from deterministic
Storybook application stories. `screenshot-manifest.json` is the source contract
for their filenames, gallery order, titles, captions, story IDs, viewport sizes,
operator mode, and any deterministic interactions.

Run:

```sh
npm run test:marketing-screenshots
```

This builds static Storybook, captures every manifest entry, writes inspection
candidates under `.artifacts/test/marketing-screenshots/storybook`, and compares
them with the reviewed PNGs without modifying tracked files.

Recreate the complete reviewed gallery with the root command:

```sh
npm run screenshots:marketing
```

After visually inspecting every generated image, run the non-mutating comparison
gate:

```sh
npm run test:marketing-screenshots
```

The capture gate fixes the clock, waits for Storybook's documentation-ready
signal and web fonts, disables animation, rejects live REST or WebSocket access,
checks image dimensions and rendered content, and rejects unreviewed pixel drift.
All generated scratch files and reports stay within the canonical `.artifacts`
tree.

The public landing-page gallery consumes this manifest and these reviewed images
directly. CI runs `npm run screenshots:marketing`, uploads the generated
`marketing-screenshots` artifact, and downloads that exact artifact before
assembling the public Pages site. Marketing screenshots remain separate from
`docs/help/screenshot-manifest.json`; Help and manual images are refreshed through
their own reviewed workflow.

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
