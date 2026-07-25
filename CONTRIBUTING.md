# Contributing to ToskLight

Thank you for contributing to ToskLight.

ToskLight is professional lighting-control software. Contributions should
preserve the operator model, show-file compatibility, and parity between the
software UI, command line, OSC, and hardware-control surfaces where applicable.

## Contribution License

By submitting a contribution to ToskLight, you agree to the contribution terms
in the ToskLight Community License.

In short: you keep ownership of your contribution, but you grant Tobias S.
Keller and any later designated ToskLight copyright holder broad rights to use,
modify, publish, sublicense, relicense, sell, and include your contribution in
ToskLight and related products, including commercial hardware products.

Accepted contributions are published under the ToskLight Community License
unless otherwise agreed in writing.

## Published Modifications

If you modify ToskLight and provide, distribute, install, or otherwise make that
modified version available to someone else, you must visibly and publicly
publish the complete source code of that modified version under the ToskLight
Community License, at no charge.

The preferred way to publish modified source is a public fork of the ToskLight
repository.

## Not a Contribution

If you send something that is not intended as a contribution, mark it clearly:

```text
Not a Contribution
```

Do this before or at the time of submission.

## Third-Party Material

Do not submit third-party code, generated code, fixture data, manuals, media,
icons, models, fonts, or other assets unless you clearly identify their source
and license.

Do not submit material under GPL, AGPL, LGPL-only, non-commercial,
no-derivatives, or proprietary terms unless it has been explicitly approved in
writing.

## Engineering Expectations

Before submitting a change, run the smallest relevant checks for the area you
changed. Common checks include:

```sh
npm run test:unit
npm run test:e2e-api
npm run test:e2e-ui
npm run test:e2e -- tests/<focused-spec>.spec.ts
```

Keep changes focused, preserve existing show compatibility unless a migration
is explicitly agreed, and update documentation or tests when behavior changes.

## Repository and release model

Forgejo is authoritative for `main`, development branches, and contributor
commits. Its push mirror sends those refs to GitHub. GitHub Actions owns all
testing, production builds, GitHub Pages deployment, semantic version
selection, `vX.Y.Z` release tags, generated release notes, hosted GitHub
Releases, and release assets.

GitHub never creates or pushes a branch commit back to Forgejo. A release tag
points to the exact mirrored commit tested by GitHub Actions. The deployed
universal mirror reconciliation imports eligible GitHub-created tags that
already point into Forgejo branch history before the next outward mirror; no
Forgejo credential is stored in GitHub.

Release versions follow Conventional Commits:

- `fix:` and `perf:` produce a patch release.
- `feat:` produces a minor release.
- `type!:` / `type(scope)!:` or a `BREAKING CHANGE:` footer produces a major
  release.
- `docs:`, `test:`, `style:`, `refactor:`, `build:`, `ci:`, and `chore:` do
  not release by default.

The first hosted release intentionally starts at `v0.1.0`, matching the current
product manifests. The release preview uses an ephemeral local `v0.0.0` history
baseline to prevent semantic-release from incorrectly choosing its default
`v1.0.0`; that baseline is deleted before publication and is never pushed.
After the first release, the latest reachable `vX.Y.Z` tag is the authoritative
product version.

The root `package.json` is private release tooling, not an npm product. Its
fixed `0.0.0` version is intentionally non-authoritative, and nothing is
published to npm. Release builds receive the proposed version through
`LIGHT_RELEASE_VERSION` / `LIGHT_MANUAL_VERSION`, so archive names, Tauri
bundle metadata, the manual, and GitHub Pages agree without changing tracked
manifests or creating a release commit. Releases do not update a changelog.

On every mirrored branch push, GitHub runs formatting, Clippy, frontend
typechecks/builds, Rust and frontend unit tests, architecture checks, the
Playwright API/UI/supplemental suites, the performance mutation gate, the
macOS desktop smoke test, manual generation, and the production archive
matrix. A trusted `main` push additionally generates the help screenshots and
product-demo video. GitHub Pages consumes those visual artifacts and the
tested manual. The serialized release job runs only after every required job
and the Pages assembly succeed; Pages deploys only after the release
transaction completes.

Preview the next version and generated notes without creating a tag, release,
commit, changelog, or tracked-file change:

```sh
npm ci
npm run release:dry-run
git diff --exit-code
```

The preview uses a disposable local bare Git remote and does not require a
GitHub or Forgejo credential. Production publication runs only in the trusted
GitHub Actions release job with its job-scoped `GITHUB_TOKEN` and
`contents: write`; all other jobs remain read-only.
