# Manual and Help Screenshots

This engineering guide covers the build and review workflow for the operator manual and in-app
Help. It is deliberately outside `docs/help`: that tree is published to desk users.

Run `npm run manual` to build and verify the PDF and offline HTML manual from the numbered Markdown
below `docs/help`. The command invokes the pinned `@tobisk/markdown-manuals` package through `npx`
and provisions its Python preprocessing/verification environment under
`.artifacts/cache/manual-venv`. Its outputs are the PDF, HTML directory, and deployable ZIP under
`.artifacts/generated/manual/`. Set `LIGHT_MANUAL_RENDERER_PACKAGE` only when deliberately testing
a different published renderer version.

Help screenshots are declared in `docs/help/screenshot-manifest.json`. Use
`npm run screenshots:help` to regenerate candidates from the Storybook and live-desk capture paths,
then review and commit accepted images. Release CI consumes the reviewed tracked set; it does not
capture screenshots during release packaging. See `docs/engineering/build-and-test-commands.md` for
the individual screenshot commands, artifact paths, and verification ladder.

When editing operator Help, keep a single first-level heading per numbered page and validate local
links and images with `npm run manual`. Keep build commands, screenshot-capture mechanics, coverage
matrices, and test implementation details in engineering documentation rather than in the manual.
