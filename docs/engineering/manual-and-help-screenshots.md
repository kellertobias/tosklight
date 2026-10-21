# Manual and Help Screenshots

`docs/help` is the shared source for in-app Help and the operator PDF/HTML manual. Keep one H1 per
page, exact UI labels, short task-oriented instructions and local illustration links. Detailed API,
wire-format and implementation material belongs in `docs/engineering`, outside the handbook.

## Generate and review

1. Run `npm run screenshots:help` and `npm run screenshots:marketing`. Their manifests own the
   generated galleries; do not commit generated PNGs. Source diagrams under
   `docs/help/assets/diagrams` are authored assets and are committed.
2. Review every new subject at its intended manual size. Use configured production components for
   examples, clearly documented offline fixtures, eight controls in full playback banks, and
   close-ups for individual controls. A generic component palette is not a product screenshot.
3. Run `node tools/check-help-screenshot-set.mjs`, then `LIGHT_MANUAL_VERSION=<version> npm run manual`.
   Inspect rendered PDF pages as well as automated image/link checks.

The pinned `@tobisk/markdown-manuals` renderer builds PDF, offline HTML and its ZIP below
`.artifacts/generated/manual`. Its Python environment is under `.artifacts/cache/manual-venv`.
Set `LIGHT_MANUAL_RENDERER_PACKAGE` only when deliberately testing another renderer version.
The build rasterizes source SVG diagrams at2× under the manual artifact tree and mirrors ignored
PNGs into Help assets for the pinned PDF renderer. The manual hook selects those PNGs for PDF/HTML;
in-app Help retains the authored SVG.

## Publish a current-main manual

Release documentation remains tied to the released application. Its PDF job waits for the complete
fresh gallery, and each capture job uploads only its own manifest-owned files. This prevents stale
tracked PNGs in one job from overwriting another job's fresh captures.

For an editorial update between releases, use **Publish current operator manual**
(`manual-pages.yml`). It rebuilds current-source HTML and PDF with a separately uploaded capture
archive; it does not relabel application binaries or performance results as a new release.

1. Finish and commit the reviewed source, fetch current upstream, integrate and push to main, and
   confirm CI for that exact hash. Keep generated PNGs out of the commit.
2. At that exact commit, run `python3 tools/manual-captures.py pack <archive-path>` using a destination
   below the canonical artifact tree. Name it `manual-captures-<40-character-source-sha>.zip`.
   The packer records every Help/marketing capture and native source still with its SHA256.
3. Upload it as a uniquely named asset on the current GitHub release. Do not replace another
   commit's archive. Dispatch `manual-pages.yml` on main with `capture_release`, `capture_asset`
   and the archive's `capture_sha256`.
4. The workflow rejects a wrong source commit, changed manifest, extra files or altered bytes before
   restoring captures. It builds and verifies the manual, reuses the existing site, replaces the
   manual subtree/PDF and refreshes the reviewed screenshot gallery. Legal and other site pages
   remain present.
5. Verify the deploy job actually ran. Fetch `/manual-publication.json` from the public site,
   check `sourceCommit`, then verify the published HTML, PDF and new images against its SHA256s.
   A skipped release workflow is not publication evidence.

The site is `https://kellertobias.github.io/tosklight/`; the online handbook is `/manual/`, and its
PDF is `/tosklight-manual.pdf`. The report records the capture archive digest and all published
manual image hashes. CI also retains the `operator-manual-current` artifact for download.

See [Build and test commands](build-and-test-commands.md) for individual capture commands and
canonical output paths. Neither gallery is a visual-regression baseline: checks reject blank,
mis-sized and error-producing renders, followed by human visual review.
