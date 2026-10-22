# Application release version policy

Forgejo owns `main`. Its existing mirror sends the tested commit to GitHub, where the gated,
serialized release job creates only a `vVERSION` tag and the application release with its built
assets. No version commit, tracked manifest update, or source history rewrite is part of release.
The mirror must import GitHub-created tags on commits reachable from Forgejo before pushing
outward again; a reset therefore has to reconcile obsolete application tags on both hosts.

`release-policy.json` starts the current application release line after the reviewed source commit
`1892d41b90f112a999acb6a2cf98054b531ffdd6`. The first release-worthy change after that baseline
produces exactly **1.0.0**, including when historical 0.x tags remain. Earlier features and breaking
changes do not trigger a release when the new commit only changes documentation or screenshots.

The maintained Conventional Commits analyzer still decides whether a change releases. `fix:` and
`perf:` release, `feat:` releases, and a breaking change releases; the first such result becomes a
major step from the retained 0.x history to 1.0.0. After the new 1.0.0 tag exists, the normal patch,
minor, and major rules resume. `docs:`, `test:`, `style:`, `refactor:`, `build:`, `ci:`, and `chore:`
do not release by themselves. Artifact and build metadata use the same selected version.

Historical 1.x/2.x application tags are incompatible with this reset. Metadata selection and release
verification fail closed if any such ancestor tag survives or returns through a mirror/full-history
checkout. The policy never deletes, moves, or pushes a tag. Old 0.x tags and nonversion documentation
capture tags remain outside the new release line. A higher new tag without the initial v1.0.0 is
also rejected. Missing or unrelated baseline history is an error, not permission to guess a version.

After the separately authorized remote cleanup, `npm run release:dry-run` previews the release using
a disposable local remote. It does not publish. `node --test tools/release-epoch.test.mjs` exercises
real semantic-release dry runs against disposable repositories, including preserved old tags,
documentation-only changes, the exact initial version, ordinary subsequent version increments,
and stale-tag rejection. CI publication still waits for every existing test/build dependency and
verifies that the release tag targets the newest tested `main` commit.
