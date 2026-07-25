# Post-release Performance Status

## Result

Implemented in `0a8fe08b feat(ci): report published release performance`.

- The Linux release archive contains the release-built benchmark executable.
- Release publication completes before the performance job starts.
- The job downloads the published artifact and tests 1,024 fixtures across 32 full universes at
  100 Hz.
- A passing baseline triggers a second 2,048-fixture density probe.
- Performance produces `healthy`, `degraded`, or `unknown` without failing the release.
- Detailed reports are attached to the GitHub Release and the status is rendered on GitHub Pages.
- Main-branch builds without a new semantic release reuse the current release status.

Verification included benchmark unit tests, release-workflow contract tests, architecture checks,
release-mode execution, YAML parsing, Pages rendering, formatting, and repository Clippy.
