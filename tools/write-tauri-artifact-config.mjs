#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { artifactPaths, repositoryRoot } from "./artifact-paths.mjs";

const application = process.argv[2];
const destination = process.argv[3];
const releaseVersion = process.env.LIGHT_RELEASE_VERSION;
const frontendDist = {
  control: artifactPaths.controlFrontend,
  hardware: artifactPaths.hardwareFrontend,
  "viz-editor": artifactPaths.vizEditorFrontend,
}[application];

if (!frontendDist || !destination) {
  console.error("usage: write-tauri-artifact-config.mjs {control|hardware|viz-editor} OUTPUT");
  process.exit(2);
}
if (
  releaseVersion &&
  !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/u.test(releaseVersion)
) {
  console.error(`error: LIGHT_RELEASE_VERSION is not valid SemVer: ${releaseVersion}`);
  process.exit(2);
}
// The Viz editor packages two things it does not own: the shipped fixture packages, and the
// generated demo show. The checked-in config names them relative to itself, which is only correct
// while the artifact root is the default one; naming them here keeps a build with
// LIGHT_ARTIFACTS_DIR set packaging the demo that build actually generated.
const resources =
  application === "viz-editor"
    ? {
        bundle: {
          resources: {
            [path.join(repositoryRoot, "assets", "fixture-library")]: "fixture-library/",
            [path.join(artifactPaths.demoShow, "demo-show.show")]: "demo-show/demo-show.show",
          },
        },
      }
    : {};
const config = {
  ...(releaseVersion ? { version: releaseVersion } : {}),
  ...resources,
  // Repository build commands create the frontend exactly once before invoking Tauri. The
  // checked-in config retains beforeBuildCommand for direct Tauri use; this generated overlay
  // disables it only for the prebuilt repository workflow.
  build: { frontendDist, beforeBuildCommand: "" },
};
fs.mkdirSync(path.dirname(destination), { recursive: true });
fs.writeFileSync(destination, `${JSON.stringify(config, null, 2)}\n`);
