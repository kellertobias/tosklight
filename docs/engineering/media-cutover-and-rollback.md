# Media Server cutover and rollback

This runbook moves one installation from the frozen C++/openFrameworks Media Server to the Media
Server owned by this repository. The two implementations never share a live library during the
change. Cutover uses a copy, so rollback remains a stop/start decision rather than a file-recovery
exercise.

## Before cutover

Do not freeze the reference repository until all of these are true:

1. The maintainer has watched the diagnostic pattern on every intended output and accepted its
   monitor, resolution, orientation, presentation rate, and colour.
2. The Media branch is integrated into `main`, and the Media build and smoke-test steps in the
   `.github/workflows/release.yml` platform matrix are green for macOS Apple Silicon, Windows
   x86_64, Linux x86_64, and Linux aarch64.
3. Every row in `media-migration-ledger.md` records the maintainer acceptance rather than
   `pending review`.
4. The original library and configuration have a recoverable backup. The source checkout at
   `/Users/keller/repos/media` is not that backup: its `media/` directory is operator data.
5. The old server is stopped. The two servers must not contend for Art-Net, sACN, CITP, HTTP, or
   the output monitor during the actual change.

The reference executable remains
`/Users/keller/repos/media/build/bin/server-core.app/Contents/MacOS/server-core` until the
maintainer performs the archival step.

## Rehearse on a copy

Create a cutover directory outside both Git repositories and copy the legacy installation's
`media/` directory into it as `media/`. Preserve `.info`, `.text-sources.json`, source media, and
the numbered folders. Never use the only copy of operator data for a rehearsal.

Run the new executable with that cutover directory as its working directory and without a
`MEDIA_CONFIG` override. Its default library root is `media`, and its default configuration path is
`media/media-server.json`. On the first run only, it adopts the legacy `.text-sources.json`; every
lossy text conversion and any moved blank-sentinel address is reported in the server log. Once the
new configuration exists, later starts never re-import the legacy text document over operator
changes.

Before opening outputs, validate the configuration:

```sh
cd /path/to/cutover-directory
/path/to/media-server --check-configuration
```

Then start the server, open its administration URL, and complete these checks:

- **Library:** use **Import all** for sources waiting to be converted. Wait for every visible job
  to finish, then confirm folder names, addresses, thumbnails, and representative still/video
  playback. Imports preserve the original source beside the HAP Alpha playback clip.
- **Text:** read every migration warning, confirm the reported address moves and simplifications,
  and display representative static text, clock, and countdown sources.
- **Network and outputs:** set the intended listeners and output identities, save, restart, and
  confirm the stored and resolved addresses. Output identities and bound listeners intentionally
  take effect only after restart.
- **Desk path:** send the real Art-Net or sACN patch, confirm the selected protocol/universe/start
  address in DMX diagnostics, and check exact raw values, winning source, rate, and staleness.
- **CITP:** confirm discovery and the advertised TCP listener use port 4809 unless the operator
  explicitly configured another TCP port; download the generated GDTF and view a live preview.
- **Picture:** run `--test-pattern` and then representative media, masks, text, and visualizers on
  every output. This final visual judgement belongs to the maintainer.

Record the accepted target commit and the configuration/library backup location in the operational
change record. Do not put operator paths or content into Git.

## Cut over

After the rehearsal is accepted, stop the reference server, start the accepted target build from
the cutover directory, and repeat the network, desk, CITP, and picture checks. Keep the original
operator data untouched for the rollback window. From this point, make library and text changes
only in the new cutover copy; there is deliberately no bidirectional synchronization.

The ToskLight repository becomes the source of truth only after the accepted Media commit is on
`main` and the supported-platform workflow is green. A local topic branch is not a completed
cutover.

## Roll back

If any acceptance check fails:

1. Stop the new Media Server so it releases every listener and output surface.
2. Preserve its configuration, logs, and copied library for diagnosis; do not copy normalized clips
   or migrated text back over the legacy installation.
3. Start the reference executable against the untouched original operator data.
4. Confirm its HTTP interface, DMX listener, CITP discovery, and program output before returning it
   to service.
5. Record the failed target commit and symptom. Fix and rehearse again on a fresh copy when the
   failure may have changed operator data.

Rollback restores the previous process and its original data. It does not attempt to translate
edits made after cutover back into the old schema.

## Archive the reference

After maintainer acceptance and integration of the target into `main`, add a prominent notice at
the top of the legacy repository's `README.md` stating that development moved to the ToskLight
repository and linking to the Media application there. Commit only that notice, create an annotated
archival tag on that commit, and push the commit and tag. The legacy repository remains buildable
and available; it is never deleted.

The legacy checkout may contain unrelated staged and unstaged work. Inspect it before the README
commit and use a path-limited commit so none of that work is accidentally included. Creating the
notice and tag is the maintainer-authorized freeze action; no remote deletion or repository
removal is required.
