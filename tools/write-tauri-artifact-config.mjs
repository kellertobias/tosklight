#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

const application = process.argv[2];
const destination = process.argv[3];
const releaseVersion = process.env.LIGHT_RELEASE_VERSION;
const desktopSidecarDirectory = process.env.LIGHT_DESKTOP_SIDECAR_DIR;
const desktopSidecarTarget = process.env.LIGHT_DESKTOP_SIDECAR_TARGET;
const frontendDirectory = {
	control: artifactPaths.controlFrontend,
	hardware: artifactPaths.hardwareFrontend,
	"viz-editor": artifactPaths.vizEditorFrontend,
}[application];
const crateDirectory = {
	control: "apps/light-desktop/src-tauri",
	hardware: "apps/light-hardware-controls/src-tauri",
	"viz-editor": "apps/viz-editor/src-tauri",
}[application];

if (!frontendDirectory || !destination) {
	console.error(
		"usage: write-tauri-artifact-config.mjs {control|hardware|viz-editor} OUTPUT",
	);
	process.exit(2);
}
if (
	releaseVersion &&
	!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/u.test(
		releaseVersion,
	)
) {
	console.error(
		`error: LIGHT_RELEASE_VERSION is not valid SemVer: ${releaseVersion}`,
	);
	process.exit(2);
}
// The Viz editor packages two things it does not own: the shipped fixture packages, and the
// generated demo show. The checked-in config names them relative to itself, which is only correct
// while the artifact root is the default one; naming them here keeps a build with
// LIGHT_ARTIFACTS_DIR set packaging the demo that build actually generated.
const bundle =
	application === "viz-editor"
		? {
				bundle: {
					resources: {
						[path.join(repositoryRoot, "assets", "fixture-library")]:
							"fixture-library/",
						[path.join(artifactPaths.demoShow, "demo-show.show")]:
							"demo-show/demo-show.show",
					},
				},
			}
		: application === "control" && desktopSidecarDirectory
			? {
					bundle: {
						externalBin: [
							path.join(desktopSidecarDirectory, "light-headless"),
							path.join(desktopSidecarDirectory, "viz-renderer"),
						],
					},
				}
			: {};
if (desktopSidecarDirectory && !desktopSidecarTarget) {
	console.error(
		"error: LIGHT_DESKTOP_SIDECAR_TARGET is required with LIGHT_DESKTOP_SIDECAR_DIR",
	);
	process.exit(2);
}
if (application === "control" && desktopSidecarDirectory) {
	const extension = desktopSidecarTarget.includes("windows") ? ".exe" : "";
	for (const binary of ["light-headless", "viz-renderer"]) {
		const source = path.join(
			desktopSidecarDirectory,
			`${binary}-${desktopSidecarTarget}${extension}`,
		);
		if (!fs.existsSync(source)) {
			console.error(`error: desktop sidecar is missing: ${source}`);
			process.exit(2);
		}
	}
}
// Relative to the crate, with forward slashes, exactly like the checked-in configs. Tauri reads
// frontendDist as a URL when it can, and a Windows absolute path is a URL: `D:\...` parses as the
// `d:` scheme, so the build embeds nothing and the window opens on a bare file:// directory.
const frontendDist = path
	.relative(path.join(repositoryRoot, crateDirectory), frontendDirectory)
	.split(path.sep)
	.join("/");
const config = {
	...(releaseVersion ? { version: releaseVersion } : {}),
	...bundle,
	// Repository build commands create the frontend exactly once before invoking Tauri. The
	// checked-in config retains beforeBuildCommand for direct Tauri use; this generated overlay
	// disables it only for the prebuilt repository workflow.
	build: { frontendDist, beforeBuildCommand: "" },
};
fs.mkdirSync(path.dirname(destination), { recursive: true });
fs.writeFileSync(destination, `${JSON.stringify(config, null, 2)}\n`);
